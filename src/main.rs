use std::collections::BTreeMap;
use std::env;
use std::sync::Arc;
use std::sync::Mutex;

use bitcoin::{secp256k1, Network, NetworkKind};
use miniscript::Descriptor;

use bdk_core::{ConfirmationBlockTime, Merge};

use bdk_chain::{
    bdk_core, bitcoin, keychain_txout::KeychainTxOutIndex, local_chain::LocalChain, miniscript,
    DescriptorExt, TxGraph,
};
use clap::Parser;
use rand::Fill;

use loon::bitcoincore_rpc::RpcApi;
use loon::{
    bitcoincore_rpc, nostr_prelude::*, rusqlite, Account, BdkChainWallet, BdkChangeSet,
    Coordinator, Friend, Keychain, BDK_CHAIN_DB_PATH, DB_PATH,
};

use cli::{Args, Cmd, GenerateSubCmd, WalletSubCmd};
use cmd::{bail, Context};

mod cli;
mod cmd;

#[tokio::main]
async fn main() -> cmd::Result<()> {
    let args = Args::parse();

    // Handle db command or generate keys
    match args.cmd {
        Cmd::Db(_) => {
            cmd::db::execute(&args.cmd)?;
            return Ok(());
        }
        Cmd::Generate(cmd) => match cmd {
            GenerateSubCmd::Nsec => {
                let keys = Keys::generate();
                println!("{}", keys.public_key.to_bech32()?);
                println!("{}", keys.secret_key().to_bech32()?);
                return Ok(());
            }
            GenerateSubCmd::Wif { test } => {
                let network = if test {
                    NetworkKind::Test
                } else {
                    NetworkKind::Main
                };
                let mut buf = [0x00; 32];
                buf.try_fill(&mut rand::thread_rng())?;

                let inner = secp256k1::SecretKey::from_slice(&buf)?;
                let prv = bitcoin::PrivateKey {
                    compressed: true,
                    network,
                    inner,
                };
                println!("{}", prv.to_wif());
                return Ok(());
            }
        },
        _ => {}
    }

    // Get descriptors from loon db
    let acct_id = args.account_id.unwrap_or(3);
    let db = rusqlite::Connection::open(DB_PATH)?;

    let mut stmt = db.prepare("SELECT * FROM account WHERE id = ?1")?;
    let mut rows = stmt.query_map([&acct_id], |row| {
        Ok(Account {
            id: row.get(0)?,
            network: row.get(1)?,
            nick: row.get(2)?,
            descriptor: row.get(3)?,
        })
    })?;
    let acct = match rows.next() {
        Some(acct) => acct?,
        None => {
            bail!("no account found for that acct id");
        }
    };

    let (desc, change_desc) = split_desc(&acct.descriptor);
    if change_desc.is_empty() {
        bail!("descriptor must be multipath");
    }
    let secp = secp256k1::Secp256k1::new();
    let desc = Descriptor::parse_descriptor(&secp, &desc)?.0;
    let change_desc = Descriptor::parse_descriptor(&secp, &change_desc)?.0;
    let did = desc.descriptor_id().to_string();
    let quorum_fp = &did[..8];

    let (network, rpc_port) = match acct.network.as_str() {
        "signet" => (Network::Signet, 38332),
        "bitcoin" => (Network::Bitcoin, 8332),
        _ => bail!("unsupported network"),
    };

    // Get friends from loon db
    let mut stmt = db.prepare("SELECT * FROM friend WHERE account_id = ?1")?;
    let friends = stmt.query_map([acct.id], |row| {
        Ok(Friend {
            account_id: row.get(0)?,
            quorum_id: row.get(1)?,
            npub: row.get(2)?,
            alias: row.get(3)?,
        })
    })?;

    // Load Bdk chain wallet for the intended quorum
    // TODO: the path to the wallet should match the account id of the quorum we're loading
    let mut conn = rusqlite::Connection::open(BDK_CHAIN_DB_PATH)?;
    let mut tx = conn.transaction()?;
    let changeset = BdkChangeSet::initialize(&mut tx)?;
    tx.commit()?;

    let BdkChangeSet {
        chain: chain_changeset,
        tx_graph: tx_graph_changeset,
        indexer,
    } = changeset.unwrap_or_default();

    let mut stage = BdkChangeSet::default();

    // Initialize chain from the network defined genesis hash
    // (staging the initial changeset), or directly from the changeset.
    let chain = if chain_changeset.is_empty() {
        let (chain, change) =
            LocalChain::from_genesis_hash(bitcoin::constants::genesis_block(network).block_hash());
        stage.merge(change.into());
        chain
    } else {
        LocalChain::from_changeset(chain_changeset)?
    };
    // Initialize txout index
    let mut index = KeychainTxOutIndex::<Keychain>::default();
    assert!(index.insert_descriptor(Keychain::EXTERNAL, desc)?);
    assert!(index.insert_descriptor(Keychain::INTERNAL, change_desc)?);
    // Initialize tx graph
    let tx_graph = TxGraph::<ConfirmationBlockTime>::default();

    let mut wallet = BdkChainWallet {
        network,
        chain,
        tx_graph,
        index,
        stage,
    };

    // reindex and apply changes
    wallet.index.apply_changeset(indexer);
    wallet.index_tx_graph_changeset(&tx_graph_changeset);
    wallet.tx_graph.apply_changeset(tx_graph_changeset);

    // Configure core rpc
    let url = format!("http://127.0.0.1:{rpc_port}");
    let cookie_file = env::var("RPC_COOKIE").context("must set RPC_COOKIE")?;
    let auth = bitcoincore_rpc::Auth::CookieFile(cookie_file.into());
    let rpc_client = bitcoincore_rpc::Client::new(&url, auth)?;

    // Configure nostr client if needed
    let client = match &args.cmd {
        Cmd::Call(_) | Cmd::Fetch { .. } | Cmd::Wallet(WalletSubCmd::Whoami) => {
            let nsec = Keys::parse(&env::var("NOSTR_NSEC").context("must set NOSTR_NSEC")?)?;
            let client = Client::builder().signer(nsec).build();
            client.add_relay("wss://relay.damus.io").await?;
            Some(Arc::new(client))
        }
        _ => None,
    };

    let db = Arc::new(Mutex::new(conn));

    // init coordinator
    let mut coordinator = Coordinator {
        fingerprint: quorum_fp.to_string(),
        wallet,
        db,
        participants: BTreeMap::new(),
        client,
        rpc_client,
    };
    // add quorum participants
    for friend_res in friends {
        let f = friend_res?;
        coordinator.add_participant(f.quorum_id, f);
    }
    // Persist the just staged change if this is the first time
    // creating a wallet
    coordinator.persist()?;

    match args.cmd {
        Cmd::Db(_) => unreachable!("handled above"),
        Cmd::Desc(subcmd) => cmd::descriptor::execute(&coordinator, subcmd)?,
        Cmd::Call(subcmd) => cmd::call::push(&coordinator, subcmd).await?,
        Cmd::Fetch { listen } => {
            if listen {
                cmd::fetch::listen(&coordinator).await?;
            } else {
                cmd::fetch::fetch_and_decrypt(&coordinator).await?;
            }
        }
        Cmd::Hash => {
            println!("{}", coordinator.rpc_client().get_best_block_hash()?)
        }
        Cmd::Generate(..) => unreachable!("handled above"),
        Cmd::Wallet(subcmd) => cmd::wallet::execute(&mut coordinator, subcmd).await?,
    }

    Ok(())
}

/// Split descriptor.
fn split_desc(desc: &str) -> (String, String) {
    use regex_lite::Captures;
    use regex_lite::Regex;
    let re = Regex::new(r"<([\d+]);([\d+])>").unwrap();
    if !re.is_match(desc) {
        return (desc.to_string(), String::new());
    }

    // find, replace
    let rep = |caps: &Captures| -> String { caps.get(1).unwrap().as_str().to_string() };
    let descriptor = re.replace_all(desc, &rep).to_string();
    let rep = |caps: &Captures| -> String { caps.get(2).unwrap().as_str().to_string() };
    let change_descriptor = re.replace_all(desc, &rep).to_string();

    (descriptor, change_descriptor)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn split_descriptor() {
        let desc = "wsh(multi(2,[7d94197e/84h/1h/0h]tpubDCmcN1ucMUfxxabEnLKHzUbjaxg8P4YR4V7mMsfhnsdRJquRyDTudrBmzZhrpV4Z4PH3MjKKFtBk6WkJbEWqL9Vc8E8v1tqFxtFXRY8zEjG/<0;1>/*,[9aa5b7ee/84h/1h/0h]tpubDCUB1aBPqtRaVXRpV6WT8RBKn6ZJhua9Uat8vvqfz2gD2zjSaGAasvKMsvcXHhCxrtv9T826vDpYRRhkU8DCRBxMd9Se3dzbScvcguWjcqF/<0;1>/*))";
        let (desc, change_desc) = split_desc(desc);
        assert!(!change_desc.is_empty());
        for s in [desc, change_desc] {
            assert!(!s.is_empty());
            assert!(!s.contains(";"));
            assert!(!s.contains("<"));
            assert!(!s.contains(">"));
        }
    }
}

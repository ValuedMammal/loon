use std::env;

use bdk_wallet::bitcoin::Network;
use clap::Parser;
use loon::bitcoincore_rpc;
use loon::db;
use loon::nostr::*;
use loon::rusqlite;
use loon::Coordinator;
use loon::BDK_DB_PATH;
use loon::DB_PATH;

use cli::{Args, Cmd};
use cmd::Context;

mod cli;
mod cmd;

#[tokio::main]
async fn main() -> cmd::Result<()> {
    let args = Args::parse();

    // Handle db command or generate nostr keys
    match args.cmd {
        Cmd::Db(_) => {
            cmd::db::execute(&args.cmd)?;
            return Ok(());
        }
        Cmd::Keys => {
            let keys = Keys::generate();
            println!("{}", keys.public_key().to_bech32()?);
            println!("{}", keys.secret_key()?.to_bech32()?);
            return Ok(());
        }
        _ => {}
    }

    // Get descriptors from loon db
    let acct_id = args.account_id.unwrap_or(1);
    let db = rusqlite::Connection::open(DB_PATH)?;

    let mut stmt = db.prepare("SELECT * FROM account WHERE id = ?1")?;
    let mut rows = stmt.query_map([&acct_id], |row| {
        Ok(db::Account {
            id: row.get(0)?,
            network: row.get(1)?,
            nick: row.get(2)?,
            descriptor: row.get(3)?,
        })
    })?;
    let acct = match rows.next() {
        Some(acct) => acct?,
        None => {
            cmd::bail!("no account found for that acct id");
        }
    };

    let (desc, change_desc) = split_desc(&acct.descriptor);
    if change_desc.is_empty() {
        cmd::bail!("descriptor must be multipath");
    }

    let (network, rpc_port) = match acct.network.as_str() {
        "signet" => (Network::Signet, 38332),
        "bitcoin" => (Network::Bitcoin, 8332),
        _ => cmd::bail!("unsupported network"),
    };

    // Get friends from loon db
    let mut stmt = db.prepare("SELECT * FROM friend WHERE account_id = ?1")?;
    let friends = stmt.query_map([acct.id], |row| {
        Ok(db::Friend {
            account_id: row.get(0)?,
            quorum_id: row.get(1)?,
            npub: row.get(2)?,
            alias: row.get(3)?,
        })
    })?;

    // Load bdk store for the provided quorum
    let mut conn = rusqlite::Connection::open(BDK_DB_PATH)?;
    let wallet = match bdk_wallet::LoadParams::new().load_wallet(&mut conn)? {
        Some(wallet) => wallet,
        None => bdk_wallet::CreateParams::new(desc, change_desc)
            .network(network)
            .create_wallet(&mut conn)?,
    };

    // Configure core rpc
    let url = format!("http://127.0.0.1:{rpc_port}");
    let cookie_file = env::var("RPC_COOKIE").context("must set RPC_COOKIE")?;
    let auth = bitcoincore_rpc::Auth::CookieFile(cookie_file.into());
    let core = bitcoincore_rpc::Client::new(&url, auth)?;

    // Configure nostr client
    let nsec = Keys::parse(env::var("NOSTR_NSEC").context("must set NOSTR_NSEC")?)?;
    let opt = Options::new().wait_for_send(false).timeout(cmd::TIMEOUT);
    let client = Client::with_opts(&nsec, opt);
    client.add_relay("wss://relay.damus.io").await?;

    // Create Coordinator
    let mut coordinator = Coordinator::builder()
        .wallet(wallet)
        .rpc_client(core)
        .nostr_client(client)
        .build()?;

    for friend in friends {
        let f = friend?;
        coordinator.add_participant(f.quorum_id, f);
    }

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
        Cmd::Keys => unreachable!("handled above"),
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

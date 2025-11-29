use anyhow::Context;
use bdk_chain::bdk_core;
use bdk_chain::bitcoin;
use bdk_chain::SpkIterator;
use bdk_core::BlockId;
use bitcoin::{address::FromScriptError, Address, Amount, FeeRate};
use filter_iter::FilterIter;

use loon::{simplerpc, Coordinator, Keychain, Update};

use super::Result;
use crate::cli::{AddressSubCmd, TxSubCmd, WalletSubCmd};

/// Minimum count of script pubkeys to scan with if none are revealed.
const SPK_CT: u32 = 20;

// Perform wallet operations.
pub async fn execute(coor: &mut Coordinator, subcmd: WalletSubCmd) -> Result<()> {
    let network = coor.network();

    match subcmd {
        // Address
        WalletSubCmd::Address(cmd) => match cmd {
            AddressSubCmd::New => {
                if let Some((indexed, addr)) = coor.wallet.reveal_next_address() {
                    let (keychain, index) = indexed;
                    coor.persist()?;

                    println!("({} {}) {}", keychain, index, addr);
                }
            }
            AddressSubCmd::Next => {
                if let Some((indexed, addr)) = coor.wallet.next_unused_address() {
                    let (keychain, index) = indexed;
                    coor.persist()?;

                    println!("({} {}) {}", keychain, index, addr);
                }
            }
            AddressSubCmd::Peek { index, keychain } => {
                if let Some((indexed, addr)) = coor.wallet.peek_address(keychain.into(), index) {
                    let (keychain, index) = indexed;

                    println!("({} {}) {}", keychain, index, addr);
                }
            }
            AddressSubCmd::List { keychain } => {
                let keychain: Keychain = keychain.into();

                let addrs = coor
                    .wallet()
                    .index
                    .revealed_keychain_spks(keychain)
                    .map(|(index, spk)| -> Result<_, FromScriptError> {
                        let addr = Address::from_script(&spk, network)?;
                        let is_used = coor.wallet.index.is_used(keychain, index);
                        Ok((index, addr, is_used))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                for (index, addr, is_used) in addrs {
                    println!("({} {}) {} used:{}", keychain, index, addr, is_used);
                }
            }
        },
        // Balance
        WalletSubCmd::Balance => display_balance(coor)?,
        // Tx
        WalletSubCmd::Tx(cmd) => match cmd {
            // List transactions by txid
            TxSubCmd::List => {
                for canon_tx in coor.wallet().transactions() {
                    // TODO: maybe display more tx details (sent, received, etc).
                    println!("Txid: {}", canon_tx.tx_node.txid);
                }
            }
            // Txout
            TxSubCmd::Out { unspent } => {
                for (indexed, txo) in coor.wallet.list_indexed_txouts() {
                    let (keychain, index) = indexed;
                    if let Some((_, addr)) = coor.wallet.peek_address(keychain, index) {
                        let is_spent = txo.spent_by.is_some();
                        if unspent && is_spent {
                            continue;
                        } else {
                            // (k, i) | amount | outpoint | address | spent
                            let op = txo.outpoint;
                            let amt = txo.txout.value;
                            println!(
                                "({} {}) {} {} {} spent:{}",
                                keychain, index, amt, op, addr, is_spent
                            );
                        }
                    }
                }
            }
            // New
            TxSubCmd::New {
                recipient,
                value,
                feerate,
                sweep,
            } => {
                let address = recipient.require_network(network)?;
                let amount = Amount::from_sat(value);
                let feerate = FeeRate::from_sat_per_kwu((feerate * 250.0).round() as u64);

                let psbt = coor.wallet.create_psbt(address, amount, feerate, sweep)?;

                dbg!(&psbt);
                println!("{}", psbt);
            }
        },
        // Display the person alias for the current user.
        WalletSubCmd::Whoami => {
            let my_pk = coor.signer().await?.get_public_key().await?;

            let (pid, p) = coor
                .participants()
                .find(|(_pid, p)| p.pk == my_pk)
                .expect("must find participant");

            println!("{}: {}", pid, p.alias.clone().unwrap_or("None".to_string()));
        }
        // Sync to chain tip
        WalletSubCmd::Sync { start } => {
            if let Some(height) = start {
                // We want to insert a block if we haven't reached the start height to prevent
                // scanning the entire chain.
                if height > coor.wallet().tip().height() {
                    let hash = coor.rpc_client().get_block_hash(height as _)?;
                    let block = BlockId { height, hash };
                    let wallet = coor.wallet_mut();
                    wallet.insert_checkpoint(block).map_err(|e| anyhow::anyhow!("{e}"))?;
                }
            }

            let mut spks = vec![];
            for (keychain, desc) in coor.wallet().index.keychains() {
                let last_reveal = coor
                    .wallet()
                    .index
                    .last_revealed_index(keychain)
                    .unwrap_or_default()
                    .max(SPK_CT);
                spks.extend(SpkIterator::new_with_range(desc, 0..=last_reveal).map(|(_, s)| s));
            }

            let mut cp = coor.wallet().tip();
            let start_height = cp.height();
            let rpc_client = get_rpc_client(network)?;
            let filter_iter = FilterIter::new(&rpc_client, cp.clone(), spks);
            let mut new_tip = cp.block_id();

            for result in filter_iter {
                let event = result?;
                let block_id = event.cp.block_id();
                let height = block_id.height;
                // Add matching blocks to tip (including those that may have been reorganized).
                if height <= start_height || event.is_match() {
                    cp = cp.insert(block_id);
                }
                // Apply matching blocks
                if let Some(ref block) = event.block {
                    coor.wallet_mut().apply_block_relevant(block, height);
                    println!("Matched block {height}");
                } else if height % 100 == 0 {
                    println!("Scanning.. {height}");
                }
                new_tip = block_id;
            }

            // Also include the new tip.
            cp = cp.insert(new_tip);

            // Apply chain update.
            coor.wallet.apply_update(Update {
                cp: Some(cp),
                ..Default::default()
            })?;

            coor.persist()?;

            println!("Local tip: {}\n", coor.wallet().tip().height());
            display_balance(coor)?;
        }
    }

    coor.persist()?;

    Ok(())
}

fn display_balance(coor: &Coordinator) -> Result<()> {
    let network = coor.network();
    let wallet = coor.wallet();

    let unspent: Vec<_> = wallet.list_unspent().collect();

    // List unspent.
    if !unspent.is_empty() {
        println!("Unspent");
        for (indexed, txo) in unspent {
            let (keychain, index) = indexed;
            let txout = txo.txout;
            println!(
                // (k, index) | address | value | outpoint
                "({} {}) | {} | {} | {}",
                keychain,
                index,
                Address::from_script(&txout.script_pubkey, network)?,
                txout.value,
                txo.outpoint,
            );
        }
    }

    // Display Balance.
    println!("\n{:#?}", wallet.balance());

    Ok(())
}

/// Initialize a new RPC client. This is here to avoid holding a reference to the
/// Coordinator's client while we're mutating the wallet.
fn get_rpc_client(network: bitcoin::Network) -> anyhow::Result<simplerpc::Client> {
    let rpc_port = match network {
        bitcoin::Network::Signet => 38332,
        bitcoin::Network::Bitcoin => 8332,
        _ => panic!("unsupported Network"),
    };
    let url = format!("http://127.0.0.1:{rpc_port}");
    let cookie_file = std::env::var("RPC_COOKIE").context("must set RPC_COOKIE")?;
    let cookie = std::fs::read_to_string(cookie_file)?;
    let simple_http = simplerpc::jsonrpc::simple_http::Builder::new()
        .url(&url)?
        .cookie_auth(cookie)
        .build();

    Ok(simplerpc::Client::with_transport(simple_http))
}

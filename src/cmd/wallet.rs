use std::io::Write;
use std::str::FromStr;

use bdk_bitcoind_rpc::compact_filter;
use bdk_wallet::bitcoin::Address;
use bdk_wallet::bitcoin::Amount;
use bdk_wallet::bitcoin::FeeRate;
use bdk_wallet::bitcoin::Network;
use bdk_wallet::KeychainKind;
use loon::Coordinator;

use super::Result;
use crate::cli::AddressSubCmd;
use crate::cli::TxSubCmd;
use crate::cli::WalletSubCmd;

// Perform wallet operations.
pub async fn execute(coordinator: &mut Coordinator, subcmd: WalletSubCmd) -> Result<()> {
    match subcmd {
        // Address
        WalletSubCmd::Address(cmd) => match cmd {
            AddressSubCmd::New => println!(
                "{:?}",
                coordinator
                    .wallet_mut()
                    .reveal_next_address(KeychainKind::External)
            ),
            AddressSubCmd::Next => println!(
                "{:?}",
                coordinator
                    .wallet_mut()
                    .next_unused_address(KeychainKind::External)
            ),
            AddressSubCmd::Peek { index } => println!(
                "{:?}",
                coordinator
                    .wallet_mut()
                    .peek_address(KeychainKind::External, index)
            ),
        },
        WalletSubCmd::Tx(cmd) => match cmd {
            // Create tx
            TxSubCmd::New {
                recipient,
                amount,
                mut feerate,
            } => {
                let addr = Address::from_str(&recipient)?.assume_checked();
                feerate *= 250.0; // -> sat/kwu
                let mut builder = coordinator.wallet_mut().build_tx();
                builder
                    .add_recipient(addr.script_pubkey(), Amount::from_sat(amount))
                    .fee_rate(FeeRate::from_sat_per_kwu(feerate as u64));
                let psbt = builder.finish()?;
                println!("{psbt}");
            }
            // List transactions
            TxSubCmd::List => {
                for canonical_tx in coordinator.wallet().transactions() {
                    println!("Txid: {}", canonical_tx.tx_node.txid);
                }
            }
        },
        // Display the person alias for the current user.
        WalletSubCmd::Whoami => {
            let my_pk = coordinator.keys().await?.public_key();

            let (pid, p) = coordinator
                .participants()
                .find(|(_pid, p)| p.pk == my_pk)
                .expect("must find participant");

            println!("{}: {}", pid, p.alias.clone().unwrap_or("None".to_string()));
        }
        // Sync to chain tip
        WalletSubCmd::Sync => {
            let mut req = compact_filter::Request::<KeychainKind>::new(
                coordinator.wallet().latest_checkpoint(),
            );

            for (keychain, desc) in coordinator.wallet().spk_index().keychains() {
                let last_reveal = coordinator
                    .wallet()
                    .spk_index()
                    .last_revealed_index(keychain)
                    .unwrap_or(10);
                req.add_descriptor(keychain, desc.clone(), 0..=last_reveal);
            }

            println!("Inventory");
            req.inspect_spks(|keychain, index, spk| {
                println!(
                    "{keychain:?} {index} {}",
                    Address::from_script(spk, Network::Signet).expect("valid Address")
                );
                std::io::stdout().flush().unwrap();
            });

            let mut client = req.build_client(coordinator.rpc_client());

            let compact_filter::Update {
                tip,
                indexed_tx_graph,
            } = client.sync()?;

            coordinator.wallet_mut().apply_update(bdk_wallet::Update {
                chain: Some(tip),
                graph: indexed_tx_graph.graph().clone(),
                last_active_indices: indexed_tx_graph.index.last_used_indices(),
            })?;

            println!("\nUnspent");
            let unspent: Vec<_> = coordinator.wallet().list_unspent().collect();
            for utxo in unspent {
                println!(
                    "{:?} | {} | {} | {}",
                    coordinator
                        .wallet()
                        .spk_index()
                        .index_of_spk(utxo.txout.script_pubkey.clone())
                        .unwrap(),
                    Address::from_script(&utxo.txout.script_pubkey, Network::Signet)?,
                    utxo.txout.value.to_btc(),
                    utxo.outpoint,
                );
            }
            println!("\n{:#?}", coordinator.wallet().balance());
        }
    }

    coordinator.save_wallet_changes()?;

    Ok(())
}

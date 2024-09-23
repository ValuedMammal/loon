use std::str::FromStr;

use bdk_bitcoind_rpc::bip158::Event;
use bdk_bitcoind_rpc::bip158::EventInner;
use bdk_bitcoind_rpc::bip158::FilterIter;
use bdk_chain::ConfirmationBlockTime;
use bdk_wallet::bitcoin::Address;
use bdk_wallet::bitcoin::Amount;
use bdk_wallet::bitcoin::FeeRate;
use bdk_wallet::chain::BlockId;
use bdk_wallet::chain::SpkIterator;
use bdk_wallet::KeychainKind;
use bdk_wallet::Update;
use loon::bdk_chain;
use loon::bdk_chain::keychain_txout::KeychainTxOutIndex;
use loon::bitcoincore_rpc::RpcApi;
use loon::Coordinator;

use super::Result;
use crate::cli::AddressSubCmd;
use crate::cli::TxSubCmd;
use crate::cli::WalletSubCmd;

// Perform wallet operations.
pub async fn execute(coordinator: &mut Coordinator, subcmd: WalletSubCmd) -> Result<()> {
    let network = coordinator.network();

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
            TxSubCmd::Sweep { recipient } => {
                let addr = Address::from_str(&recipient)?.assume_checked();
                let mut builder = coordinator.wallet_mut().build_tx();
                builder.drain_to(addr.script_pubkey()).drain_wallet();
                let psbt = builder.finish()?;
                println!("{psbt}");
            }
            TxSubCmd::New {
                recipient,
                amount,
                feerate,
            } => {
                let addr = Address::from_str(&recipient)?.assume_checked();
                let sat_kwu = feerate.unwrap_or(1.0) * 250.0;
                let mut builder = coordinator.wallet_mut().build_tx();
                builder
                    .add_recipient(addr.script_pubkey(), Amount::from_sat(amount))
                    .fee_rate(FeeRate::from_sat_per_kwu(sat_kwu as u64));
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
        WalletSubCmd::Sync { start } => {
            if let Some(height) = start {
                if height > coordinator.wallet().latest_checkpoint().height() {
                    // insert a block to avoid scanning the entire chain
                    let hash = coordinator.rpc_client().get_block_hash(height as u64)?;
                    let _ = coordinator
                        .wallet_mut()
                        .insert_checkpoint(BlockId { height, hash })?;
                }
            }

            let cp = coordinator.wallet().latest_checkpoint();
            let start_height = cp.height();
            let mut emitter = FilterIter::new_with_checkpoint(coordinator.rpc_client(), cp);

            for (keychain, desc) in coordinator.wallet().spk_index().keychains() {
                let last_reveal = coordinator
                    .wallet()
                    .spk_index()
                    .last_revealed_index(keychain)
                    .unwrap_or(10);
                emitter.add_spks(
                    SpkIterator::new_with_range(desc, 0..=last_reveal).map(|(_, spk)| spk),
                );
            }
            let mut tmp_graph = bdk_chain::IndexedTxGraph::<
                ConfirmationBlockTime,
                KeychainTxOutIndex<KeychainKind>,
            >::default();

            // sync
            if let Some(tip) = emitter.get_tip()? {
                let blocks_to_scan = tip.height - start_height;

                for event in emitter.by_ref() {
                    let event = event?;
                    let curr = event.height();
                    if let Event::Block(EventInner { height, ref block }) = event {
                        let _ = tmp_graph.apply_block_relevant(block, height);
                        println!("Matched block {}", curr);
                    }
                    if curr % 1000 == 0 {
                        let progress = (curr - start_height) as f32 / blocks_to_scan as f32;
                        println!("[{:.2}%]", progress * 100.0);
                    }
                }
                // apply chain update
                if let Some(tip) = emitter.chain_update() {
                    let wallet = coordinator.wallet_mut();
                    wallet.apply_update(Update {
                        chain: Some(tip),
                        ..Default::default()
                    })?;
                }
                // apply graph update
                let wallet = coordinator.wallet_mut();
                wallet.apply_update(Update {
                    tx_update: tmp_graph.graph().clone().into(),
                    ..Default::default()
                })?;
                coordinator.save_wallet_changes()?;
            }

            println!(
                "Local tip: {}",
                coordinator.wallet().latest_checkpoint().height()
            );
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
                    Address::from_script(&utxo.txout.script_pubkey, network)?,
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

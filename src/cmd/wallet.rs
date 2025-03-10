use std::str::FromStr;

use bdk_bitcoind_rpc::bip158::{Event, EventInner, FilterIter};
use bdk_wallet::bitcoin::Address;
use bdk_wallet::bitcoin::Amount;
use bdk_wallet::bitcoin::FeeRate;
use bdk_wallet::chain::BlockId;
use bdk_wallet::chain::SpkIterator;
use bdk_wallet::KeychainKind;
use bdk_wallet::Update;
use loon::bdk_chain;
use loon::bdk_chain::ConfirmationBlockTime;
use loon::bitcoincore_rpc::RpcApi;
use loon::Coordinator;

use super::Result;
use crate::cli::{AddressSubCmd, TxSubCmd, WalletSubCmd};

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
        // Balance
        WalletSubCmd::Balance => println!("{:#?}", coordinator.wallet().balance()),
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
            let my_pk = coordinator.signer().await?.get_public_key().await?;

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
                    // insert a block to prevent scanning the entire chain
                    let hash = coordinator.rpc_client().get_block_hash(height as _)?;
                    let block = BlockId { height, hash };
                    bdk_wallet::test_utils::insert_checkpoint(coordinator.wallet_mut(), block);
                }
            }

            let cp = coordinator.wallet().latest_checkpoint();
            let start_height = cp.height();

            // clone out the keychains into a temporary tx graph
            let mut tmp_graph = bdk_chain::IndexedTxGraph::<ConfirmationBlockTime, _>::new(
                coordinator.wallet().spk_index().clone(),
            );

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

            // sync
            if let Some(tip) = emitter.get_tip()? {
                let blocks_to_scan = tip.height - start_height;

                for event in emitter.by_ref() {
                    let event = event?;
                    let curr = event.height();
                    if let Event::Block(EventInner { height, ref block }) = event {
                        println!("Matched block {}", curr);
                        let _ = tmp_graph.apply_block_relevant(block, height);
                    }
                    if curr % 1000 == 0 {
                        let progress = (curr - start_height) as f32 / blocks_to_scan as f32;
                        println!("[{:.2}%]", progress * 100.0);
                    }
                }

                // apply update
                let last_active_indices = tmp_graph.index.last_used_indices();
                let tx_update = tmp_graph.graph().clone().into();
                let chain = emitter.chain_update();
                coordinator.wallet_mut().apply_update(Update {
                    last_active_indices,
                    tx_update,
                    chain,
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

use bitcoin::{address::FromScriptError, Address};

use bdk_core::{BlockId, ConfirmationBlockTime, Merge, TxUpdate};

use bdk_chain::bdk_core;
use bdk_chain::bitcoin;
use bdk_chain::SpkIterator;

use bdk_bitcoind_rpc::bip158::{Event, EventInner, FilterIter};

use loon::bitcoincore_rpc::RpcApi;
use loon::{Coordinator, Keychain, Update};

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
        // TODO: Create tx
        WalletSubCmd::Tx(cmd) => match cmd {
            // List transactions by txid
            TxSubCmd::List => {
                for canon_tx in coor.wallet().transactions() {
                    // TODO: maybe display more tx details (sent, received, etc).
                    println!("Txid: {}", canon_tx.tx_node.txid);
                }
            }
            // Sweep
            TxSubCmd::Sweep { .. } => unimplemented!(),
            // New
            TxSubCmd::New { .. } => unimplemented!(),
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

            let cp = coor.wallet().tip();
            let start_height = cp.height();

            // Clone the keychains into a temporary tx graph. This is used to aggregate relevant
            // transactions during the sync.
            use bdk_chain::indexed_tx_graph::{self, IndexedTxGraph};
            let mut tmp_graph =
                IndexedTxGraph::<ConfirmationBlockTime, _>::new(coor.wallet().index.clone());
            let mut tmp_changeset = indexed_tx_graph::ChangeSet::default();

            let mut emitter = FilterIter::new_with_checkpoint(coor.rpc_client(), cp);

            for (keychain, desc) in coor.wallet().index.keychains() {
                let last_reveal =
                    coor.wallet().index.last_revealed_index(keychain).unwrap_or(SPK_CT);
                emitter.add_spks(
                    SpkIterator::new_with_range(desc, 0..=last_reveal).map(|(_, spk)| spk),
                );
            }

            // Sync
            if let Some(tip) = emitter.get_tip()? {
                let blocks_to_scan = tip.height - start_height;

                for event in emitter.by_ref() {
                    let event = event?;
                    let curr = event.height();
                    if let Event::Block(EventInner { height, ref block }) = event {
                        println!("Matched block {}", curr);
                        tmp_changeset.merge(tmp_graph.apply_block_relevant(block, height));
                    }
                    if curr % 1000 == 0 {
                        let progress = (curr - start_height) as f32 / blocks_to_scan as f32;
                        println!("[{:.2}%]", progress * 100.0);
                    }
                }

                // Apply updates
                let last_active_indices = tmp_graph.index.last_used_indices();
                let tx_update: TxUpdate<_> = tmp_graph.graph().clone().into();
                let cp = emitter.chain_update();

                let index_changeset =
                    coor.wallet.index.reveal_to_target_multi(&last_active_indices);
                coor.wallet.stage(index_changeset);
                coor.wallet.index_tx_graph_changeset(&tmp_changeset.tx_graph);
                coor.wallet.apply_update(Update {
                    tx_update,
                    cp,
                    ..Default::default()
                })?;

                coor.persist()?;
            }

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

    // list unspent
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

    // display Balance
    println!("\n{:#?}", wallet.balance());

    Ok(())
}

use bdk_bitcoind_rpc::compact_filter;
use bdk_wallet::KeychainKind;
use loon::Coordinator;

use super::Result;

/// Filter scan
pub fn filter_scan(mut coordinator: Coordinator) -> Result<()> {
    let mut req =
        compact_filter::Request::<KeychainKind>::new(coordinator.wallet().latest_checkpoint());
    for (keychain, desc) in coordinator.wallet().spk_index().keychains() {
        req.add_descriptor(keychain, desc.clone(), 0..10);
    }
    let mut client = req.build_client(coordinator.chain());
    let compact_filter::Update {
        tip,
        indexed_tx_graph,
    } = client.sync()?;

    coordinator.wallet_mut().apply_update(bdk_wallet::Update {
        chain: Some(tip),
        graph: indexed_tx_graph.graph().clone(),
        last_active_indices: indexed_tx_graph.index.last_used_indices(),
    })?;

    coordinator.save_wallet_changes()?;

    println!("{:?}", coordinator.wallet().balance());
    for tx in coordinator.wallet().transactions() {
        dbg!(tx.tx_node.txid);
    }

    Ok(())
}

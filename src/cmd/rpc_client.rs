use bdk_bitcoind_rpc::compact_filter;
use bdk_wallet::{
    bitcoin::{Address, Network},
    KeychainKind,
};
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

    let unspent: Vec<_> = coordinator.wallet().list_unspent().collect();
    for utxo in unspent {
        println!(
            "{} | {} | {}",
            Address::from_script(&utxo.txout.script_pubkey, Network::Signet)?,
            utxo.txout.value.display_dynamic(),
            utxo.outpoint,
        );
    }

    println!("{:#?}", coordinator.wallet().balance());

    Ok(())
}

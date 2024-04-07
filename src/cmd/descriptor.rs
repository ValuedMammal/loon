use bitcoincore_rpc::bitcoincore_rpc_json;
use bitcoincore_rpc::bitcoincore_rpc_json::ImportDescriptors;
use bitcoincore_rpc::RpcApi;
use loon::Coordinator;

use crate::cli::DescSubCmd;

// Core RPCs related to descriptors, useful for quick debugging
pub fn execute(coordinator: &Coordinator, subcmd: DescSubCmd) -> super::Result<()> {
    let client = coordinator.chain();

    match subcmd {
        DescSubCmd::Import { desc, internal } => {
            let req = ImportDescriptors {
                descriptor: desc,
                timestamp: bitcoincore_rpc_json::Timestamp::Now,
                active: Some(true),
                internal: if internal { Some(true) } else { None },
                ..Default::default()
            };

            let res = client.import_descriptors(req)?;
            println!("{res:#?}");
        }
        DescSubCmd::Info { desc } => {
            let res = client.get_descriptor_info(&desc)?;
            println!("{res:#?}");
        }
    }

    Ok(())
}

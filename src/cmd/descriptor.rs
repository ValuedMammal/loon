use loon::simplerpc::types::ImportDescriptorsRequest;
use loon::Coordinator;

use crate::cli::DescSubCmd;

// Descriptor utilities.
pub fn execute(coordinator: &Coordinator, subcmd: DescSubCmd) -> super::Result<()> {
    let client = coordinator.rpc_client();

    match subcmd {
        // Import a descriptor at the specified time, or default to a time of "now".
        DescSubCmd::Import {
            desc,
            timestamp,
            internal,
        } => {
            let request = ImportDescriptorsRequest {
                desc,
                timestamp: timestamp
                    .unwrap_or_else(|| std::time::UNIX_EPOCH.elapsed().unwrap().as_secs()),
                internal: if internal { Some(true) } else { None },
                ..Default::default()
            };

            let res = client.import_descriptors(&[request])?;
            println!("{res:#?}");
        }
        // Get descriptor info.
        DescSubCmd::Info { desc } => {
            let res = client.get_descriptor_info(&desc)?;
            println!("{res:#?}");
        }
    }

    Ok(())
}

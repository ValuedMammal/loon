use bdk::wallet::AddressIndex;
use loon::Coordinator;

use crate::cli::AddressSubCmd;
use crate::cli::WalletSubCmd;

// Perform wallet operations
pub fn execute(mut coordinator: Coordinator<'_>, subcmd: WalletSubCmd) -> super::Result<()> {
    match subcmd {
        // Address
        WalletSubCmd::Address(cmd) => {
            match cmd {
                // Get new address
                AddressSubCmd::New => {
                    println!(
                        "{:?}",
                        coordinator.wallet().try_get_address(AddressIndex::New)?
                    );
                }
                AddressSubCmd::Next => todo!(),
                AddressSubCmd::Peek { .. } => todo!(),
            }
        }
    }

    Ok(())
}

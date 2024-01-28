use bdk::wallet::AddressIndex;
use loon::Coordinator;

use super::Result;
//use super::bail;
use crate::cli::AddressSubCmd;
use crate::cli::WalletSubCmd;

// Perform wallet operations.
pub async fn execute(mut coordinator: Coordinator<'_>, subcmd: WalletSubCmd) -> Result<()> {
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
        // Display the person alias for the current user.
        WalletSubCmd::Whoami => {
            let my_pk = coordinator.keys().await?.public_key();

            let (pid, p) = coordinator
                .participants()
                .find(|(_pid, p)| p.pk == my_pk)
                .expect("must find participant");

            println!("{}: {}", pid, p.alias.clone().unwrap_or("None".to_string()))
        }
    }

    Ok(())
}

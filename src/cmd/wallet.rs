use bdk::wallet::AddressIndex;
use loon::Coordinator;

use super::Result;
//use super::bail;
use crate::cli::AddressSubCmd;
use crate::cli::WalletSubCmd;

// Perform wallet operations.
pub async fn execute(mut coordinator: Coordinator, subcmd: WalletSubCmd) -> Result<()> {
    match subcmd {
        // New
        WalletSubCmd::New { descriptor } => {
            let _desc = descriptor;
            // create bdk wallet

            // parse friends from desc

            // set a nick
            // write to loon.db
            // stmt: insert into account ..
            // stmt: insert into friend ..
        }
        // Address
        WalletSubCmd::Address(cmd) => match cmd {
            AddressSubCmd::New => println!(
                "{:?}",
                coordinator.wallet().try_get_address(AddressIndex::New)
            ),
            AddressSubCmd::Next => println!(
                "{:?}",
                coordinator
                    .wallet()
                    .try_get_address(AddressIndex::LastUnused)
            ),
            AddressSubCmd::Peek { index } => println!(
                "{:?}",
                coordinator
                    .wallet()
                    .try_get_address(AddressIndex::Peek(index))
            ),
        },
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

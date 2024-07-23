use bdk_wallet::KeychainKind;
use loon::Coordinator;

use super::Result;
//use super::bail;
use crate::cli::AddressSubCmd;
use crate::cli::WalletSubCmd;

// Perform wallet operations.
pub async fn execute(coordinator: &mut Coordinator, subcmd: WalletSubCmd) -> Result<()> {
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

        // List wallet transactions
        WalletSubCmd::Transactions => {
            for canonical_tx in coordinator.wallet().transactions() {
                println!("Txid: {}", canonical_tx.tx_node.txid);
            }
        }
        // Display the person alias for the current user.
        WalletSubCmd::Whoami => {
            let my_pk = coordinator.keys().await?.public_key();

            let (pid, p) = coordinator
                .participants()
                .find(|(_pid, p)| p.pk == my_pk)
                .expect("must find participant");

            println!("{}: {}", pid, p.alias.clone().unwrap_or("None".to_string()));
        }
    }

    coordinator.save_wallet_changes()?;

    Ok(())
}

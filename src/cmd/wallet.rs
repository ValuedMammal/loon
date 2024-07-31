use std::str::FromStr;

use bdk_wallet::bitcoin::Address;
use bdk_wallet::bitcoin::Amount;
use bdk_wallet::bitcoin::FeeRate;
use bdk_wallet::KeychainKind;
use loon::Coordinator;

use super::Result;
use crate::cli::AddressSubCmd;
use crate::cli::TxSubCmd;
use crate::cli::WalletSubCmd;

// Perform wallet operations.
pub async fn execute(coordinator: &mut Coordinator, subcmd: WalletSubCmd) -> Result<()> {
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

        // List wallet transactions
        WalletSubCmd::Tx(cmd) => match cmd {
            TxSubCmd::New {
                recipient,
                amount,
                mut feerate,
            } => {
                let addr = Address::from_str(&recipient)?.assume_checked();
                feerate *= 250.0; // -> sat/kwu
                let mut builder = coordinator.wallet_mut().build_tx();
                builder
                    .add_recipient(addr.script_pubkey(), Amount::from_sat(amount))
                    .fee_rate(FeeRate::from_sat_per_kwu(feerate as u64));
                let psbt = builder.finish()?;
                println!("{psbt}");
            }
            TxSubCmd::List => {
                for canonical_tx in coordinator.wallet().transactions() {
                    println!("Txid: {}", canonical_tx.tx_node.txid);
                }
            }
        },
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

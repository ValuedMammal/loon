use std::fmt;

mod coordinator;
pub mod db;

pub use {
    bdk_bitcoind_rpc::bitcoincore_rpc,
    bdk_wallet::chain as bdk_chain,
    bdk_wallet::rusqlite,
    // Re-exports
    coordinator::{Call, CallTy, Coordinator, Participant, HRP},
    nostr_sdk::prelude as nostr,
};

/// Alias for a Bdk persisted wallet
pub type Wallet = bdk_wallet::PersistedWallet<rusqlite::Connection>;

/// Bdk wallet db path
pub const BDK_DB_PATH: &str = "./wallet.db";
/// Loon db path
pub const DB_PATH: &str = "./loon.db";

/// Crate errors.
#[derive(Debug)]
pub enum Error {
    /// Builder
    Builder,
    /// Coordinator
    Coordinator(String),
    /// Nostr client
    Nostr(nostr_sdk::client::Error),
    /// Rusqlite
    Rusqlite(rusqlite::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builder => write!(f, "not all required fields present"),
            Self::Coordinator(e) => e.fmt(f),
            Self::Nostr(e) => e.fmt(f),
            Self::Rusqlite(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

/// Chat entry.
#[derive(Debug)]
pub struct ChatEntry {
    /// Sender alias
    pub alias: String,
    /// Text message
    pub message: String,
}

#[cfg(test)]
#[allow(unused)]
mod test {
    use super::*;

    #[ignore]
    #[test]
    fn it_works() {}
}

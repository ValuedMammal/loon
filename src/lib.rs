use std::fmt;

mod coordinator;
mod db;
mod wallet;

pub use coordinator::*;
pub use db::*;
pub use wallet::*;

// Re-exports
#[cfg(feature = "nostr-sdk")]
pub use nostr_sdk;
pub use {
    bdk_chain::rusqlite,
    bitcoin::secp256k1::rand,
    filter_iter::{self, simplerpc},
};

/// BDK wallet database prefix name.
pub const BDK_DB_PREFIX: &str = "wallet";
/// Path to Loon database.
pub const LOON_DB_PATH: &str = "loon.db";
/// Human-readable part of a loon call
pub const HRP: &str = "loon1";

/// Crate error
#[derive(Debug)]
pub enum Error {
    /// Coordinator
    Coordinator(String),
    /// Nostr client
    #[cfg(feature = "nostr-sdk")]
    Nostr(nostr_sdk::client::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinator(e) => e.fmt(f),
            #[cfg(feature = "nostr-sdk")]
            Self::Nostr(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

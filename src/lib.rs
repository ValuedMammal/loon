use std::fmt;

mod coordinator;
mod db;
mod wallet;

pub use coordinator::*;
pub use db::*;
pub use wallet::*;

// Re-exports
pub use {bdk_chain::rusqlite, filter_iter::{self, simplerpc}, nostr_sdk::prelude as nostr_prelude};

/// Bdk chain db path
pub const BDK_DB_PATH: &str = "./wallet.db";
/// Loon db path
pub const DB_PATH: &str = "./loon.db";
/// Human-readable part of a loon call
pub const HRP: &str = "loon1";

/// Crate error
#[derive(Debug)]
pub enum Error {
    /// Coordinator
    Coordinator(String),
    /// Nostr client
    Nostr(nostr_sdk::client::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinator(e) => e.fmt(f),
            Self::Nostr(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

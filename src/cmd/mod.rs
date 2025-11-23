// Allow un-inlined format string args
#![allow(clippy::uninlined_format_args)]

pub mod call;
pub mod db;
pub mod descriptor;
pub mod fetch;
pub mod wallet;

pub use loon::rusqlite;
pub use nostr_sdk::prelude as nostr;

pub use anyhow::bail;
pub use anyhow::Context;
pub use anyhow::Result;

/// App default nostr client timeout
pub const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

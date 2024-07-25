pub mod call;
pub mod db;
pub mod descriptor;
pub mod fetch;
pub mod wallet;

pub use nostr_sdk::prelude as nostr;

pub use anyhow::bail;
pub use anyhow::Result;

/// App default nostr client timeout
pub const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

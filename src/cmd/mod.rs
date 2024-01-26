pub mod call;
pub mod descriptor;
pub mod wallet;

pub use nostr_sdk::prelude as nostr;

pub use anyhow::Result;
pub use anyhow::bail;

/// App default nostr client timeout
pub const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

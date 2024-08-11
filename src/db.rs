/// Represents a row in table 'account'.
#[derive(Debug)]
pub struct Account {
    pub id: u32,
    pub network: String,
    pub nick: String,
    pub descriptor: String,
}

/// Represents a row in table 'friend'.
#[derive(Debug)]
pub struct Friend {
    pub account_id: u32,
    pub quorum_id: u32,
    pub npub: String,
    pub alias: Option<String>,
}

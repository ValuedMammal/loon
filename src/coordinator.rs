use std::fmt;
use std::sync::{Arc, Mutex};

use bdk_chain::bitcoin;

#[cfg(feature = "nostr-sdk")]
use nostr_sdk::prelude::{self as nostr, *};

#[allow(unused_imports)]
use crate::Error;
use crate::{rusqlite, simplerpc, BdkWallet as Wallet};

/// Coordinator
#[derive(Debug)]
pub struct Coordinator {
    /// quorum fingerprint
    pub fingerprint: String,
    /// Bdk wallet
    pub wallet: Wallet,
    /// database connection
    pub db: Arc<Mutex<rusqlite::Connection>>,
    /// quorum participants by id
    #[cfg(feature = "nostr-sdk")]
    pub participants: std::collections::BTreeMap<Pid, Participant>,
    // Nostr client
    #[cfg(feature = "nostr-sdk")]
    pub client: Arc<nostr::Client>,
    // RPC client
    pub rpc_client: simplerpc::Client,
}

impl Coordinator {
    /// Add a `Participant`.
    #[cfg(feature = "nostr-sdk")]
    pub fn add_participant(&mut self, pid: impl Into<Pid>, participant: impl Into<Participant>) {
        self.participants.insert(pid.into(), participant.into());
    }

    /// Get the wallet network.
    pub fn network(&self) -> bitcoin::Network {
        self.wallet.network
    }

    /// Get a reference to the `Wallet`.
    pub fn wallet(&self) -> &Wallet {
        &self.wallet
    }

    /// Get a mutable reference to the `Wallet`.
    pub fn wallet_mut(&mut self) -> &mut Wallet {
        &mut self.wallet
    }

    /// Get an iterator over the quorum participants.
    #[cfg(feature = "nostr-sdk")]
    pub fn participants(&self) -> impl Iterator<Item = (&Pid, &Participant)> {
        self.participants.iter()
    }

    /// Get a reference to the nostr client.
    #[cfg(feature = "nostr-sdk")]
    pub fn client(&self) -> Arc<nostr::Client> {
        self.client.clone()
    }

    /// Get a reference to the blockchain RPC client.
    pub fn rpc_client(&self) -> &simplerpc::Client {
        &self.rpc_client
    }

    /// Get nostr signer.
    #[cfg(feature = "nostr-sdk")]
    pub async fn signer(&self) -> Result<Arc<dyn NostrSigner>, Error> {
        self.client.signer().await.map_err(Error::Nostr)
    }

    /// Returns the unique fingerprint of the active quorum.
    ///
    /// This value is defined as the first four bytes of the sha256 hash of the wallet's
    /// public descriptor.
    pub fn quorum_fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Creates a new `Call` to `recipient` with the given `payload`.
    pub fn call_new_with_recipient_and_payload(&self, recipient: Pid, payload: &str) -> Call {
        let mut call = Call::new(crate::HRP);
        call.push(self.quorum_fingerprint())
            .push(&recipient.to_string())
            .build(payload);
        call
    }

    /// Persist the changes that have been staged by the onchain wallet.
    ///
    /// Returns whether anything was persisted.
    pub fn persist(&mut self) -> Result<bool, rusqlite::Error> {
        let mut conn = self.db.lock().unwrap();
        self.wallet.persist(&mut conn).map(|c| c.is_some())
    }
}

/// A participant in a quorum.
#[derive(Debug)]
#[cfg(feature = "nostr-sdk")]
pub struct Participant {
    pub pk: nostr_sdk::PublicKey,
    pub alias: Option<String>,
    pub account_id: u32,
    pub quorum_id: Pid,
}

#[cfg(feature = "nostr-sdk")]
impl From<crate::Friend> for Participant {
    fn from(friend: crate::Friend) -> Self {
        let pk = nostr_sdk::PublicKey::from_bech32(&friend.npub).expect("must have valid npub");
        Self {
            pk,
            alias: friend.alias,
            account_id: friend.account_id,
            quorum_id: friend.quorum_id.into(),
        }
    }
}

/// Participant id, a.k.a the quorum id.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pid(u32);

impl Pid {
    /// Get the Pid as u32.
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl From<u32> for Pid {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // This allows us to parse incoming `Call`s, because we can rely
        // on a length of 2 for the pid encoding.
        let u = self.0;
        let s = if u <= 9 {
            format!("0{u}")
        } else {
            u.to_string()
        };
        s.fmt(f)
    }
}

/// Types of calls.
#[derive(Debug)]
pub enum CallTy {
    /// Nack
    Nack,
    /// Ack
    Ack,
    /// Note
    Note(String),
}

impl CallTy {
    /// The numeric id of self.
    pub fn id(&self) -> u8 {
        match self {
            Self::Nack => 0,
            Self::Ack => 1,
            Self::Note(_) => 2,
        }
    }
}

impl AsRef<str> for CallTy {
    fn as_ref(&self) -> &str {
        match self {
            Self::Nack => "Nack",
            Self::Ack => "Ack",
            Self::Note(m) => m.as_str(),
        }
    }
}

impl fmt::Display for CallTy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// A message to a quorum participant.
#[derive(Debug, Clone)]
pub struct Call(String);

impl Call {
    /// Constructs a new `Call`.
    fn new(s: &str) -> Self {
        Self(s.to_string())
    }

    /// Push an `item` onto `self`.
    fn push(&mut self, item: &str) -> &mut Self {
        self.0.push_str(item);
        self
    }

    /// Appends the `payload`.
    fn build(&mut self, payload: &str) {
        self.0.push_str(payload);
    }
}

impl fmt::Display for Call {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Chat entry.
#[derive(Debug)]
pub struct ChatEntry {
    /// Sender alias
    pub alias: String,
    /// Text message
    pub message: String,
}

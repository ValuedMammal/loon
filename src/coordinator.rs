use std::collections::BTreeMap;
use std::fmt;

use bdk_file_store::Store;
use bdk::wallet::ChangeSet;
use bdk::bitcoin::hashes::Hash;
use bdk::bitcoin::hashes::sha256;

use super::nostr;
use super::nostr::XOnlyPublicKey;
use super::nostr::FromBech32;
use crate::db;

/// Human-readable part of a loon call.
pub const HRP: &str = "loon1";

/// Coordinator
#[derive(Debug)]
pub struct Coordinator<'a> {
    // account short name
    label: String,
    // bdk Wallet
    wallet: bdk::Wallet<Store<'a, ChangeSet>>,
    // relates quorum_id to a participant
    participants: BTreeMap<Pid, Participant>,
    // nostr client
    messenger: Option<nostr::Client>,
    // source of chain data
    blockchain: Option<bitcoincore_rpc::Client>,
}

impl<'a> Coordinator<'a> {
    /// Construct a new Coordinator.
    pub fn new(label: &str, wallet: bdk::Wallet<Store<'a, ChangeSet>>) -> Self {
        Coordinator {
            label: label.to_owned(),
            wallet,
            participants: BTreeMap::new(),
            messenger: None,
            blockchain: None,
        }
    }

    /// Set nostr client.
    pub fn with_client_nostr(&mut self, client: nostr::Client) {
        self.messenger = Some(client)
    }

    /// Set RPC client.
    pub fn with_client_rpc(&mut self, client: bitcoincore_rpc::Client) {
        self.blockchain = Some(client)
    }

    /// Insert a participant.
    pub fn insert(&mut self, pid: impl Into<Pid>, participant: impl Into<Participant>) {
        self.participants.insert(pid.into(), participant.into());
    }

    /// Get a `Participant`.
    pub fn get(&self, pid: impl Into<Pid>) -> Option<&Participant> {
        self.participants.get(&pid.into())
    }

    /// Get the current `Account` nickname.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Get a mutable reference to the `Wallet`.
    pub fn wallet(&mut self) -> &mut bdk::Wallet<Store<'a, ChangeSet>> {
        &mut self.wallet
    }

    /// Get an iterator over the quorum participants.
    pub fn participants(&self) -> impl Iterator<Item = (&Pid, &Participant)> {
        self.participants.iter()
    }

    /// Get a reference to the message client.
    pub fn messenger(&self) -> Option<&nostr::Client> {
        self.messenger.as_ref()
    }

    /// Get a reference to the chain backend.
    pub fn chain(&self) -> Option<&bitcoincore_rpc::Client> {
        self.blockchain.as_ref()
    }

    /// Returns the unique fingerprint of the active quorum.
    ///
    /// This value is defined as the first four bytes of the sha256 hash of the wallet's
    /// public descriptor.
    pub fn quorum_fingerprint(&self) -> String {
        let desc = self
            .wallet
            .public_descriptor(bdk::KeychainKind::External)
            .expect("wallet descriptor");
        let hash = sha256::Hash::hash(desc.to_string().as_bytes());
        (hash.to_string()[..8]).to_string()
    }

    /// Creates a new `Call` to `recipient` with the given `payload`.
    pub fn call_new_with_recipient_and_payload(&self, recipient: Pid, payload: &str) -> Call {
        let mut call = Call::new(HRP);
        call.push(&self.quorum_fingerprint())
            .push(recipient)
            .build(payload);
        call
    }
}

/// A participant in a quorum.
#[derive(Debug)]
pub struct Participant {
    pub pk: XOnlyPublicKey,
    pub alias: Option<String>,
    pub account_id: u32,
    pub quorum_id: Pid,
}

impl From<db::Friend> for Participant {
    fn from(friend: db::Friend) -> Self {
        let pk = XOnlyPublicKey::from_bech32(friend.npub).expect("must have valid npub");
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
    pub fn as_u32(&self) -> u32 {
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
    fn new(s: impl ToString) -> Self {
        Self(s.to_string())
    }

    /// Push an `item` onto `self`.
    fn push(&mut self, item: impl ToString) -> &mut Self {
        self.0.push_str(item.to_string().as_str());
        self
    }

    /// Appends the `payload`.
    fn build(&mut self, payload: &str) {
        self.0.push_str(payload)
    }
}

impl fmt::Display for Call {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

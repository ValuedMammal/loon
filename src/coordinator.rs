use std::collections::BTreeMap;
use std::fmt;

use bdk_wallet::bitcoin::hashes::sha256;
use bdk_wallet::bitcoin::hashes::Hash;
use nostr_sdk::FromBech32;

use super::nostr;
use super::nostr::NostrSigner;
use super::Wallet;
use crate::db;
use crate::Error;
use crate::BDK_DB_PATH;

/// Human-readable part of a loon call.
pub const HRP: &str = "loon1";

/// Coordinator
#[derive(Debug)]
pub struct Coordinator {
    // account short name
    label: String,
    // bdk Wallet
    wallet: Wallet,
    // relates quorum_id to a participant
    participants: BTreeMap<Pid, Participant>,
    // nostr client
    messenger: nostr::Client,
    // source of chain data
    oracle: bitcoincore_rpc::Client,
}

impl Coordinator {
    /// Build a Coordinator from parts.
    ///
    /// See [`Builder`].
    pub fn builder(label: &str, wallet: Wallet) -> Builder {
        let mut builder = Builder::default();
        builder.label(label).wallet(wallet);
        builder
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

    /// Get a reference to the `Wallet`.
    pub fn wallet(&self) -> &Wallet {
        &self.wallet
    }

    /// Get a mutable reference to the `Wallet`.
    pub fn wallet_mut(&mut self) -> &mut Wallet {
        &mut self.wallet
    }

    /// Get an iterator over the quorum participants.
    pub fn participants(&self) -> impl Iterator<Item = (&Pid, &Participant)> {
        self.participants.iter()
    }

    /// Get a reference to the message client.
    pub fn messenger(&self) -> &nostr::Client {
        &self.messenger
    }

    /// Get a reference to the chain backend.
    pub fn chain(&self) -> &bitcoincore_rpc::Client {
        &self.oracle
    }

    /// Get nostr keys.
    pub async fn keys(&self) -> Result<nostr::Keys, Error> {
        let signer = self.messenger.signer().await.map_err(Error::Nostr)?;
        let NostrSigner::Keys(k) = signer else {
            return Err(Error::Coordinator("only Keys signers allowed".to_string()));
        };
        Ok(k)
    }

    /// Returns the unique fingerprint of the active quorum.
    ///
    /// This value is defined as the first four bytes of the sha256 hash of the wallet's
    /// public descriptor.
    pub fn quorum_fingerprint(&self) -> String {
        let desc = self
            .wallet
            .public_descriptor(bdk_wallet::KeychainKind::External);
        let hash = sha256::Hash::hash(desc.to_string().as_bytes());
        (hash.to_string()[..8]).to_string()
    }

    /// Creates a new `Call` to `recipient` with the given `payload`.
    pub fn call_new_with_recipient_and_payload(&self, recipient: Pid, payload: &str) -> Call {
        let mut call = Call::new(HRP);
        call.push(&self.quorum_fingerprint())
            .push(&recipient.to_string())
            .build(payload);
        call
    }

    /// Write changes to bdk database.
    pub fn save_wallet_changes(&mut self) -> Result<(), Error> {
        let mut conn = rusqlite::Connection::open(BDK_DB_PATH).map_err(Error::Rusqlite)?;
        self.wallet.persist(&mut conn).map_err(Error::Rusqlite)?;
        Ok(())
    }
}

/// Builder.
#[derive(Debug, Default)]
pub struct Builder {
    label: Option<String>,
    wallet: Option<Wallet>,
    messenger: Option<nostr::Client>,
    oracle: Option<bitcoincore_rpc::Client>,
}

impl Builder {
    /// Setter for label.
    fn label(&mut self, label: &str) -> &mut Self {
        self.label = Some(label.to_string());
        self
    }

    /// Setter for wallet.
    fn wallet(&mut self, wallet: Wallet) -> &mut Self {
        self.wallet = Some(wallet);
        self
    }

    /// Setter for nostr client.
    pub fn with_nostr(&mut self, client: nostr::Client) -> &mut Self {
        self.messenger = Some(client);
        self
    }

    /// Setter for chain oracle.
    pub fn with_oracle(&mut self, client: bitcoincore_rpc::Client) -> &mut Self {
        self.oracle = Some(client);
        self
    }

    /// Finish building and return a new Coordinator.
    pub fn build(self) -> Result<Coordinator, Error> {
        if self.label.is_none()
            || self.wallet.is_none()
            || self.messenger.is_none()
            || self.oracle.is_none()
        {
            return Err(Error::Builder);
        }

        Ok(Coordinator {
            label: self.label.unwrap(),
            wallet: self.wallet.unwrap(),
            participants: BTreeMap::new(),
            messenger: self.messenger.unwrap(),
            oracle: self.oracle.unwrap(),
        })
    }
}

/// A participant in a quorum.
#[derive(Debug)]
pub struct Participant {
    pub pk: nostr_sdk::PublicKey,
    pub alias: Option<String>,
    pub account_id: u32,
    pub quorum_id: Pid,
}

impl From<db::Friend> for Participant {
    fn from(friend: db::Friend) -> Self {
        let pk = nostr_sdk::PublicKey::from_bech32(friend.npub).expect("must have valid npub");
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

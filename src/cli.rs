use bitcoin::{address::NetworkUnchecked, Address};
use clap::Parser;
use clap::Subcommand;

#[derive(Parser)]
#[clap(author, about, version)]
pub struct Args {
    /// Account id
    #[clap(long, short)]
    pub account_id: Option<u32>,
    #[clap(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Push notes.
    #[clap(subcommand)]
    #[cfg(feature = "nostr-sdk")]
    Call(CallSubCmd),
    /// Database operations.
    #[clap(subcommand)]
    Db(DbSubCmd),
    /// Descriptors operations.
    #[clap(subcommand)]
    Desc(DescSubCmd),
    /// Fetch notes from quorum participants.
    #[cfg(feature = "nostr-sdk")]
    Fetch {
        /// Poll for new notes continuously.
        #[clap(long, short = 'l')]
        listen: bool,
    },
    /// Get best block hash
    Hash,
    /// Generate a keypair
    #[clap(subcommand)]
    Generate(GenerateSubCmd),
    /// Wallet operations.
    #[clap(subcommand)]
    Wallet(WalletSubCmd),
}

#[derive(Subcommand)]
#[cfg(feature = "nostr-sdk")]
pub enum CallSubCmd {
    /// Construct a new private note.
    New(CallOpt),
    /// Push a plain text note.
    Push {
        /// Text
        #[clap(required = true)]
        note: String,
    },
}

#[derive(Parser)]
pub struct CallOpt {
    #[clap(flatten)]
    pub recipient: Recipient,
    /// Text
    #[clap(long, short = 'm')]
    pub note: Option<String>,
    /// Affirmative
    #[clap(long, short = 'a')]
    pub ack: bool,
    /// Negative
    #[clap(long, short = 'n')]
    pub nack: bool,
    /// Preview a call without sending
    #[clap(long, short = 'd')]
    pub dryrun: bool,
}

#[derive(Subcommand)]
pub enum DescSubCmd {
    /// Import a descriptor to Bitcoin Core
    Import {
        /// Descriptor string
        #[arg(required = true)]
        desc: String,
        /// Time when the descriptor became active. [default: "now"].
        #[clap(long, short)]
        timestamp: Option<u64>,
        /// Denotes the descriptor should only be used for change.
        #[clap(long, short)]
        internal: bool,
    },
    /// Get descriptor info
    Info {
        /// Descriptor string
        #[arg(required = true)]
        desc: String,
    },
}

#[derive(Subcommand)]
pub enum DbSubCmd {
    /// Add new quorum account
    Account {
        /// Network
        #[arg(required = true)]
        network: String,
        /// Account nickname
        #[arg(required = true)]
        nick: String,
        /// Descriptor
        #[clap(required = true)]
        descriptor: String,
    },
    /// Add new participant to existing quorum
    Friend {
        /// Account id
        #[clap(required = true)]
        account_id: u32,
        /// Quorum id
        #[clap(required = true)]
        quorum_id: u32,
        /// Nostr npub
        #[clap(required = true)]
        npub: String,
        /// Participant alias
        #[clap(required = true)]
        alias: String,
    },
}

#[derive(Subcommand)]
pub enum GenerateSubCmd {
    /// Generate nostr keys
    #[cfg(feature = "nostr-sdk")]
    Nsec,
    /// Generate a random WIF private key
    Wif {
        /// Specifies that the key is valid for test networks. If none specified, use mainnet
        /// network kind
        #[clap(long, short)]
        test: bool,
    },
}

#[derive(Subcommand)]
pub enum WalletSubCmd {
    /// Address
    #[clap(subcommand)]
    Address(AddressSubCmd),
    /// Get wallet balance
    Balance,
    /// Sync with blockchain
    Sync {
        /// Begin scan from height
        #[clap(long)]
        start: Option<u32>,
    },
    /// Transactions
    #[clap(subcommand)]
    Tx(TxSubCmd),
    /// Display the alias for the current user.
    #[cfg(feature = "nostr-sdk")]
    Whoami,
}

#[derive(Subcommand)]
pub enum AddressSubCmd {
    /// List addresses
    List {
        /// Keychain to list addresses from
        #[clap(long, short, default_value = "0")]
        keychain: u8,
    },
    /// New address
    New,
    /// Next unused
    Next,
    /// Peek at a given keychain index
    Peek {
        /// Address index
        #[clap(required = true)]
        index: u32,
        /// Keychain
        #[clap(long, short, default_value = "0")]
        keychain: u8,
    },
}

#[derive(Subcommand)]
pub enum TxSubCmd {
    /// Create new
    New {
        /// Recipient address
        #[clap(required = true)]
        recipient: Address<NetworkUnchecked>,
        /// Amount to send in satoshis
        #[clap(required = true)]
        value: u64,
        /// Feerate (sat/vb)
        #[clap(long, short, default_value = "1.2")]
        feerate: f32,
        /// Send all
        #[clap(long, short, default_value = "false")]
        sweep: bool,
    },
    /// List transactions
    List,
    /// List tx outputs
    Out {
        /// List unspent
        #[clap(long, short)]
        unspent: bool,
    },
}

#[derive(Parser)]
pub struct Recipient {
    /// Participant id
    #[clap(long)]
    pub id: Option<u32>,
    /// Recipient alias
    #[clap(long, short = 't')]
    pub alias: Option<String>,
}

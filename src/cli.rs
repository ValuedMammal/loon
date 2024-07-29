use clap::Parser;
use clap::Subcommand;

#[derive(Parser)]
#[clap(author, about)]
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
    Call(CallSubCmd),
    /// Database operations.
    #[clap(subcommand)]
    Db(DbSubCmd),
    /// Descriptors operations.
    #[clap(subcommand)]
    Desc(DescSubCmd),
    /// Fetch notes from quorum participants.
    Fetch {
        /// Poll for new notes continuously.
        #[clap(long, short = 'l')]
        listen: bool,
    },
    /// Generate new nostr keys.
    Keys,
    /// Sync
    Sync,
    /// Wallet operations.
    #[clap(subcommand)]
    Wallet(WalletSubCmd),
}

#[derive(Subcommand)]
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
    /// Import descriptors to Bitcoin Core
    Import {
        /// Descriptor
        #[arg(required = true)]
        desc: String,
        /// Internal flag
        #[clap(long, short = 'i')]
        internal: bool,
    },
    /// Get descriptor info
    Info {
        /// Descriptor
        #[arg(required = true)]
        desc: String,
    },
}

#[derive(Subcommand)]
pub enum DbSubCmd {
    /// Add new quorum account
    Account {
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
        quid: u32,
        /// Nostr npub
        #[clap(required = true)]
        npub: String,
        /// Participant alias
        #[clap(required = true)]
        alias: String,
    },
}

#[derive(Subcommand)]
pub enum WalletSubCmd {
    /// Address
    #[clap(subcommand)]
    Address(AddressSubCmd),
    /// Transactions
    Transactions,
    /// Display the alias for the current user.
    Whoami,
}

#[derive(Subcommand)]
pub enum AddressSubCmd {
    /// New address
    New,
    /// Next unused
    Next,
    /// Peek at a given index
    Peek {
        /// Address index
        #[clap(required = true)]
        index: u32,
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

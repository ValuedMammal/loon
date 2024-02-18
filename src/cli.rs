use clap::Parser;
use clap::Subcommand;

/// Loon - Coordination at a distance.
#[derive(Parser)]
#[clap(author, about)]
pub struct Args {
    /// Account nickname
    #[clap(long, short = 'n')]
    pub nick: Option<String>,
    #[clap(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Push notes.
    #[clap(subcommand)]
    Call(CallSubCmd),
    /// Descriptors operations.
    #[clap(subcommand)]
    Desc(DescSubCmd),
    /// Fetch notes from quorum participants.
    Fetch {
        /// Poll for new notes continuously.
        #[clap(long, short = 'l')]
        listen: bool,
    },
    // PSBT operations.
    //Psbt,
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
pub enum WalletSubCmd {
    /// Address
    #[clap(subcommand)]
    Address(AddressSubCmd),
    // Psbt (new, combine)
    //Psbt,
    // Load, unload wallet
    //Load,
    //Unload,
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

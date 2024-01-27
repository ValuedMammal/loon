use std::env;
use std::path;

use bdk::Wallet;
use bdk::bitcoin::Network;
use bdk_file_store::Store;
use clap::Parser;
use loon::Coordinator;
use loon::db;
use nostr_sdk::prelude::*;

mod cli;
mod cmd;

use cli::{Args, Cmd};

#[tokio::main]
async fn main() -> cmd::Result<()> {
    let args = Args::parse();

    // Configure core rpc
    let url = "http://127.0.0.1:38332";
    let user = env::var("RPC_USER")?;
    let pass = env::var("RPC_PASS")?;
    let auth = bitcoincore_rpc::Auth::UserPass(user, pass);
    let core = bitcoincore_rpc::Client::new(url, auth)?;

    // Configure db
    let db_path = "./loon.db";
    let db = rusqlite::Connection::open(db_path).expect("db connect");

    // Configure nostr client
    let nsec = Keys::from_sk_str(&env::var("NOSTR_NSEC").expect("keys from env"))?;
    let opt = Options::new().wait_for_send(false).timeout(cmd::TIMEOUT);
    let client = Client::with_opts(&nsec, opt);
    client.add_relay("wss://relay.damus.io").await?;

    // Get account nick from user
    let nick = args.nick.unwrap_or("test".to_string());

    // Get descriptors from loon db
    let mut stmt = db.prepare("SELECT * FROM account WHERE nick = ?1")?;
    let mut rows = stmt.query_map([&nick], |row| {
        Ok(db::Account {
            id: row.get(0)?,
            nick: row.get(1)?,
            descriptor: row.get(2)?,
        })
    })?;
    let acct = match rows.next() {
        Some(acct) => acct?,
        None => {
            cmd::bail!("no account found for that nick");
        }
    };
    let descriptor = acct.descriptor;

    // Get friends from loon db
    let mut stmt = db.prepare("SELECT * FROM friend WHERE account_id = ?1")?;
    let friends = stmt.query_map([acct.id], |row| {
        Ok(db::Friend {
            account_id: row.get(0)?,
            quorum_id: row.get(1)?,
            npub: row.get(2)?,
            alias: row.get(3)?,
        })
    })?;

    // Load bdk store for the provided quorum
    let store =
        Store::open_or_create_new("bdk-store".as_bytes(), path::PathBuf::from("./wallet.db"))?;
    let wallet = Wallet::new_or_load(&descriptor, None, store, Network::Signet)?;

    // Create Coordinator
    let mut coordinator = Coordinator::new(&nick, wallet);
    coordinator.with_client_nostr(client);
    coordinator.with_client_rpc(core);

    // fill participants
    for friend in friends {
        let f = friend?;
        coordinator.insert(f.quorum_id, f);
    }

    match args.cmd {
        Cmd::Desc(subcmd) => cmd::descriptor::execute(coordinator, subcmd)?,
        Cmd::Call(param) => cmd::call::push_with_options(coordinator, param).await?,
        Cmd::Fetch => cmd::call::fetch_and_decrypt(coordinator).await?,
        Cmd::Push { note } => cmd::call::push(coordinator, &note).await?,
        Cmd::Wallet(subcmd) => cmd::wallet::execute(coordinator, subcmd)?,
    }

    Ok(())
}

use std::env;

use bdk_wallet::bitcoin::Network;
use bdk_wallet::Wallet;

use bdk_sqlite::Store;
use clap::Parser;
use loon::db;
use loon::Coordinator;
use nostr_sdk::prelude::*;

mod cli;
mod cmd;

use cli::{Args, Cmd};

#[tokio::main]
async fn main() -> cmd::Result<()> {
    let args = Args::parse();

    // Configure core rpc
    let url = "http://127.0.0.1:38332"; // signet
    let cookie_file = env::var("RPC_COOKIE")?;
    let auth = bitcoincore_rpc::Auth::CookieFile(cookie_file.into());
    let core = bitcoincore_rpc::Client::new(url, auth)?;

    // Configure db
    let db_path = "./loon.db";
    let db = rusqlite::Connection::open(db_path)?;

    // Configure nostr client
    let nsec = Keys::parse(env::var("NOSTR_NSEC").expect("keys from env"))?;
    let opt = Options::new().wait_for_send(false).timeout(cmd::TIMEOUT);
    let client = Client::with_opts(&nsec, opt);
    client.add_relay("wss://relay.damus.io").await?;

    // Get account nick from user
    // TODO: handle case where `nick` is None, i.e. we want to create a new wallet.
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
    let conn = bdk_sqlite::rusqlite::Connection::open("./wallet.db")?;
    let mut store = Store::new(conn)?;
    let changeset = store.read()?;
    // FIXME
    let change_desc = "wpkh(cVpPVruEDdmutPzisEsYvtST1usBR3ntr8pXSyt6D2YYqXRyPcFW)".to_string();
    let wallet = Wallet::new_or_load(&descriptor, &change_desc, changeset, Network::Signet)?;

    // Create Coordinator
    let mut builder = Coordinator::builder(&nick, wallet);
    builder.with_nostr(client).with_oracle(core);
    let mut coordinator = builder.build()?;
    for friend in friends {
        let f = friend?;
        coordinator.insert(f.quorum_id, f);
    }

    match args.cmd {
        Cmd::Desc(subcmd) => cmd::descriptor::execute(&coordinator, subcmd)?,
        Cmd::Call(subcmd) => cmd::call::push(&coordinator, subcmd).await?,
        Cmd::Fetch { listen } => {
            if listen {
                cmd::fetch::listen(&coordinator).await?;
            } else {
                cmd::fetch::fetch_and_decrypt(&coordinator).await?;
            }
        }
        Cmd::Wallet(subcmd) => cmd::wallet::execute(&mut coordinator, subcmd).await?,
    }

    Ok(())
}

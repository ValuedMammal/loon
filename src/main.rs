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
    let (desc, change_desc) = split_desc(&acct.descriptor);
    if change_desc.is_empty() {
        panic!("descriptor must be multipath");
    }

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
    let wallet = Wallet::new_or_load(&desc, &change_desc, changeset, Network::Signet)?;

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

/// Split descriptor.
#[allow(unused)]
fn split_desc(desc: &str) -> (String, String) {
    use regex_lite::Captures;
    use regex_lite::Regex;
    let re = Regex::new(r"<([\d+]);([\d+])>").unwrap();
    if !re.is_match(&desc) {
        return (desc.to_string(), String::new());
    }

    // we have a match
    let caps = re.captures(&desc).unwrap();

    // find, replace
    let rep = |caps: &Captures| -> String { caps.get(1).unwrap().as_str().to_string() };
    let descriptor = re.replace_all(&desc, &rep).to_string();
    let rep = |caps: &Captures| -> String { caps.get(2).unwrap().as_str().to_string() };
    let change_descriptor = re.replace_all(&desc, &rep).to_string();

    (descriptor, change_descriptor)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn split_descriptor() {
        let desc = "wsh(multi(2,[7d94197e/84h/1h/0h]tpubDCmcN1ucMUfxxabEnLKHzUbjaxg8P4YR4V7mMsfhnsdRJquRyDTudrBmzZhrpV4Z4PH3MjKKFtBk6WkJbEWqL9Vc8E8v1tqFxtFXRY8zEjG/<0;1>/*,[9aa5b7ee/84h/1h/0h]tpubDCUB1aBPqtRaVXRpV6WT8RBKn6ZJhua9Uat8vvqfz2gD2zjSaGAasvKMsvcXHhCxrtv9T826vDpYRRhkU8DCRBxMd9Se3dzbScvcguWjcqF/<0;1>/*))";
        let (desc, change_desc) = split_desc(desc);
        assert!(!change_desc.is_empty());
        for s in [desc, change_desc] {
            assert!(!s.is_empty());
            assert!(!s.contains(";"));
            assert!(!s.contains("<"));
            assert!(!s.contains(">"));
        }
    }
}

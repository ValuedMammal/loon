use super::rusqlite;
use super::rusqlite::named_params;
use crate::cli::Cmd;
use crate::cli::DbSubCmd;

/// Execute database operation.
pub fn execute(cmd: &Cmd) -> anyhow::Result<()> {
    if let Cmd::Db(cmd) = cmd {
        let db = rusqlite::Connection::open(loon::LOON_DB_PATH)?;

        match cmd {
            // Insert into account
            DbSubCmd::Account {
                network,
                nick,
                descriptor,
            } => {
                let mut stmt = db.prepare(
                    "INSERT INTO account (network, nick, descriptor) VALUES (:network, :nick, :descriptor)",
                )?;
                let ct = stmt.execute(
                    named_params! {":network": network, ":nick": nick, ":descriptor": descriptor},
                )?;
                println!("Inserted {ct} rows into table account");

                // get current acct id
                let mut stmt = db.prepare("SELECT max(id) FROM account")?;
                let id = stmt.query_row([], |row| row.get::<usize, usize>(0))?;
                println!("Row id {id}");
            }
            // Insert into friend
            DbSubCmd::Friend {
                account_id,
                quorum_id,
                npub,
                alias,
            } => {
                let mut stmt = db.prepare("INSERT INTO friend (account_id, quorum_id, npub, alias) VALUES (:account_id, :quorum_id, :npub, :alias)")?;
                let ct =
                    stmt.execute(named_params! {":account_id": account_id, ":quorum_id": quorum_id, ":npub": npub, ":alias": alias})?;
                println!("Inserted {ct} rows into table friend");
            }
        }
    }

    Ok(())
}

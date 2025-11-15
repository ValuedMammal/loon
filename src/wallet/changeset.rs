use bdk_core::{ConfirmationBlockTime, Merge};

use bdk_chain::{bdk_core, keychain_txout, local_chain, rusqlite, tx_graph};

/// Bdk change set
#[derive(Debug, Clone, Default)]
pub struct BdkChangeSet {
    /// local chain
    pub chain: local_chain::ChangeSet,
    /// tx graph
    pub tx_graph: tx_graph::ChangeSet<ConfirmationBlockTime>,
    /// indexer
    pub indexer: keychain_txout::ChangeSet,
}

impl BdkChangeSet {
    /// Initialize a changeset with the provided rusqlite `tx`, or return `None` if the changeset
    /// is empty.
    pub fn initialize(tx: &mut rusqlite::Transaction) -> Result<Option<Self>, rusqlite::Error> {
        local_chain::ChangeSet::init_sqlite_tables(tx)?;
        tx_graph::ChangeSet::init_sqlite_tables(tx)?;
        keychain_txout::ChangeSet::init_sqlite_tables(tx)?;

        let chain = local_chain::ChangeSet::from_sqlite(tx)?;
        let tx_graph = tx_graph::ChangeSet::from_sqlite(tx)?;
        let indexer = keychain_txout::ChangeSet::from_sqlite(tx)?;

        let changeset = Self {
            chain,
            tx_graph,
            indexer,
        };

        if changeset.is_empty() {
            Ok(None)
        } else {
            Ok(Some(changeset))
        }
    }

    /// Persist `self` to SQLite
    pub fn persist(&self, tx: &mut rusqlite::Transaction) -> Result<(), rusqlite::Error> {
        self.chain.persist_to_sqlite(tx)?;
        self.tx_graph.persist_to_sqlite(tx)?;
        self.indexer.persist_to_sqlite(tx)?;

        Ok(())
    }
}

impl Merge for BdkChangeSet {
    fn merge(&mut self, other: Self) {
        self.chain.merge(other.chain);
        self.tx_graph.merge(other.tx_graph);
        self.indexer.merge(other.indexer);
    }

    fn is_empty(&self) -> bool {
        self.chain.is_empty() && self.tx_graph.is_empty() && self.indexer.is_empty()
    }
}

impl From<local_chain::ChangeSet> for BdkChangeSet {
    fn from(chain: local_chain::ChangeSet) -> Self {
        Self {
            chain,
            ..Default::default()
        }
    }
}

impl From<tx_graph::ChangeSet<ConfirmationBlockTime>> for BdkChangeSet {
    fn from(tx_graph: tx_graph::ChangeSet<ConfirmationBlockTime>) -> Self {
        Self {
            tx_graph,
            ..Default::default()
        }
    }
}

impl From<keychain_txout::ChangeSet> for BdkChangeSet {
    fn from(indexer: keychain_txout::ChangeSet) -> Self {
        Self {
            indexer,
            ..Default::default()
        }
    }
}

impl From<(keychain_txout::ChangeSet, tx_graph::ChangeSet<ConfirmationBlockTime>)>
    for BdkChangeSet
{
    fn from(
        (indexer, tx_graph): (
            keychain_txout::ChangeSet,
            tx_graph::ChangeSet<ConfirmationBlockTime>,
        ),
    ) -> Self {
        Self {
            indexer,
            tx_graph,
            ..Default::default()
        }
    }
}

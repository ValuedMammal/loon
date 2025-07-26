use std::cmp;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use bitcoin::key::rand::Rng;
use bitcoin::{
    absolute, key::rand, transaction, Address, Amount, FeeRate, Network, OutPoint, Psbt, Sequence,
    Transaction,
};
use miniscript::plan::{Assets, Plan};
use miniscript::ForEachKey;

use bdk_core::{BlockId, CheckPoint, ConfirmationBlockTime, Merge, TxUpdate};

use bdk_chain::{
    bdk_core, bitcoin,
    keychain_txout::KeychainTxOutIndex,
    local_chain::{CannotConnectError, LocalChain},
    miniscript, rusqlite,
    tx_graph::{self, CanonicalTx},
    Anchor, Balance, CanonicalizationParams, ChainPosition, FullTxOut, Indexer, KeychainIndexed,
    TxGraph,
};

use bdk_tx::{
    group_by_spk, CanonicalUnspents, ChangePolicyType, InputCandidates, Output, PsbtParams,
    Selector, SelectorParams, TxStatus,
};

mod changeset;
pub use changeset::*;

/// Represents the unique id of a descriptor
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Keychain(pub u8);

impl Keychain {
    /// The external keychain, used for receive addresses
    pub const EXTERNAL: Self = Self(0);
    /// The internal keychain, used for change addresses
    pub const INTERNAL: Self = Self(1);
}

impl From<u8> for Keychain {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl fmt::Display for Keychain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            k if k == Self::EXTERNAL => write!(f, "External"),
            k if k == Self::INTERNAL => write!(f, "Internal"),
            _ => self.0.fmt(f),
        }
    }
}

/// Stores and indexes on-chain data
#[derive(Debug, Clone)]
pub struct BdkWallet {
    /// network
    pub network: Network,
    /// local chain
    pub chain: LocalChain,
    /// tx graph
    pub tx_graph: TxGraph<ConfirmationBlockTime>,
    /// indexer
    pub index: KeychainTxOutIndex<Keychain>,
    // staged change set
    pub stage: BdkChangeSet,
}

impl BdkWallet {
    /// Latest checkpoint tip
    pub fn tip(&self) -> CheckPoint {
        self.chain.tip()
    }

    /// Insert checkpoint and return whether it was newly inserted.
    ///
    /// Error if trying to replace an existing block of the local chain at height of `block`.
    pub fn insert_checkpoint(&mut self, block: BlockId) -> Result<bool, String> {
        let mut cp = self.chain.tip();

        // If we already have the same block, return Ok otherwise fail if inserting
        // the given block would invalidate the original chain.
        if let Some(cp) = cp.get(block.height) {
            if cp.hash() == block.hash {
                return Ok(false);
            }
            return Err(format!(
                "Cannot replace block of original chain {} {}",
                cp.height(),
                cp.hash()
            ));
        }

        // Now we can safely insert
        cp = cp.insert(block);
        let change = self.chain.apply_update(cp).expect("should apply");
        let ret = !change.is_empty();
        self.stage(change);

        Ok(ret)
    }

    /// List unspent txouts (UTXOs)
    pub fn list_unspent(
        &self,
    ) -> impl Iterator<Item = KeychainIndexed<Keychain, FullTxOut<ConfirmationBlockTime>>> + '_
    {
        self.list_indexed_txouts().filter(|(_, txo)| txo.spent_by.is_none())
    }

    /// List indexed full txouts
    pub fn list_indexed_txouts(
        &self,
    ) -> impl Iterator<Item = KeychainIndexed<Keychain, FullTxOut<ConfirmationBlockTime>>> + '_
    {
        let chain = &self.chain;
        let chain_tip = chain.tip().block_id();
        let outpoints = self.index.outpoints().clone();

        self.tx_graph.filter_chain_txouts(
            chain,
            chain_tip,
            CanonicalizationParams::default(),
            outpoints,
        )
    }

    /// Retrieve the balance
    pub fn balance(&self) -> Balance {
        let chain = &self.chain;
        let chain_tip = chain.tip().block_id();
        let outpoints = self.index.outpoints().clone();

        self.tx_graph.balance(
            chain,
            chain_tip,
            CanonicalizationParams::default(),
            outpoints,
            |&(k, _), _| k == Keychain::INTERNAL,
        )
    }

    /// List wallet transactions
    pub fn transactions(
        &self,
    ) -> impl Iterator<Item = CanonicalTx<Arc<Transaction>, ConfirmationBlockTime>> {
        self.tx_graph.list_canonical_txs(
            &self.chain,
            self.tip().block_id(),
            CanonicalizationParams::default(),
        )
    }

    /// Reveal next receive address. You must [`persist`](Self::persist) the staged changes.
    pub fn reveal_next_address(&mut self) -> Option<KeychainIndexed<Keychain, Address>> {
        let keychain = Keychain::EXTERNAL;
        let ((index, spk), change) = self.index.reveal_next_spk(keychain)?;
        let addr = Address::from_script(&spk, self.network).ok()?;
        self.stage(change);

        Some(((keychain, index), addr))
    }

    /// Next unused receive address. You must [`persist`](Self::persist) the staged changes.
    pub fn next_unused_address(&mut self) -> Option<KeychainIndexed<Keychain, Address>> {
        let keychain = Keychain::EXTERNAL;
        let ((index, spk), change) = self.index.next_unused_spk(keychain)?;
        let addr = Address::from_script(&spk, self.network).ok()?;
        self.stage(change);

        Some(((keychain, index), addr))
    }

    /// Peek address of the given keychain at index.
    ///
    /// None if `index` is in the hardened derivation range (>= 2^31).
    pub fn peek_address(
        &self,
        keychain: Keychain,
        index: u32,
    ) -> Option<KeychainIndexed<Keychain, Address>> {
        use bitcoin::bip32::ChildNumber;
        let _idx = ChildNumber::from_normal_idx(index).ok()?;
        let desc = self.index.get_descriptor(keychain)?;
        let spk = desc
            .at_derivation_index(index)
            .expect("must be valid derivation index")
            .script_pubkey();
        let addr = Address::from_script(&spk, self.network).ok()?;

        Some(((keychain, index), addr))
    }

    /// Apply an [`Update`]. This stages the change to be persisted later.
    ///
    /// Errors if the chain update fails.
    pub fn apply_update(&mut self, update: impl Into<Update>) -> Result<(), CannotConnectError> {
        let Update {
            tx_update,
            cp,
            last_active_indices,
        } = update.into();

        let mut changeset = BdkChangeSet::default();

        // index
        changeset.merge(self.index.reveal_to_target_multi(&last_active_indices).into());

        // chain
        if let Some(cp) = cp {
            changeset.merge(self.chain.apply_update(cp)?.into());
        }

        // tx graph
        changeset.merge(self.tx_graph.apply_update(tx_update).into());

        self.stage(changeset);

        Ok(())
    }

    /// Indexes the txs and txouts of `tx_graph` changeset and stages the resulting changes.
    ///
    /// This is necessary to discover or replenish the set of indexed outputs controlled by the
    /// wallet.
    pub fn index_tx_graph_changeset(
        &mut self,
        tx_graph: &tx_graph::ChangeSet<ConfirmationBlockTime>,
    ) {
        let index = &mut self.index;
        let mut change = BdkChangeSet::default();

        for tx in &tx_graph.txs {
            change.merge(index.index_tx(tx).into());
        }
        for (&op, txout) in &tx_graph.txouts {
            change.merge(index.index_txout(op, txout).into());
        }

        self.stage(change);
    }

    /// Stage a change set.
    pub fn stage(&mut self, changeset: impl Into<BdkChangeSet>) {
        self.stage.merge(changeset.into());
    }

    /// Persist the staged changes and return the old stage, if any.
    pub fn persist(
        &mut self,
        conn: &mut rusqlite::Connection,
    ) -> Result<Option<BdkChangeSet>, rusqlite::Error> {
        let mut tx = conn.transaction()?;

        let mut ret = None;

        if self.staged().is_some() {
            self.stage.persist(&mut tx)?;
            tx.commit()?;
            ret = self.stage.take();
        }

        Ok(ret)
    }

    /// See the staged changes, if any.
    pub fn staged(&mut self) -> Option<&BdkChangeSet> {
        if self.stage.is_empty() {
            None
        } else {
            Some(&self.stage)
        }
    }

    /// Assets
    fn assets(&self) -> Assets {
        let mut v = vec![];
        for (_, desc) in self.index.keychains() {
            desc.for_each_key(|k| {
                v.push(k.clone());
                true
            });
        }
        Assets::new().add(v)
    }

    /// Return the input candidates
    fn input_candidates(&self) -> bdk_tx::InputCandidates {
        let assets = self.assets();
        let chain = &self.chain;
        let indexer = &self.index;
        let txs_with_status = self
            .tx_graph
            .list_canonical_txs(chain, chain.tip().block_id(), CanonicalizationParams::default())
            .map(|c_tx| (c_tx.tx_node.tx, status_from_position(c_tx.chain_position)));
        let canon_utxos = CanonicalUnspents::new(txs_with_status);
        let can_select = canon_utxos.try_get_unspents(
            indexer
                .outpoints()
                .iter()
                .filter_map(|&(_, op)| Some((op, try_plan(&self.index, op, &assets)?))),
        );
        InputCandidates::new([], can_select)
    }

    /// Create PSBT.
    pub fn create_psbt(
        &mut self,
        address: Address,
        amount: Amount,
        feerate: FeeRate,
    ) -> anyhow::Result<Psbt> {
        let longterm_feerate = bitcoin::FeeRate::from_sat_per_vb_unchecked(8);
        let change_keychain = Keychain::INTERNAL;
        let ((next_index, _), index_changeset) = self
            .index
            .next_unused_spk(change_keychain)
            .expect("keychain must exist");
        let change_desc = self
            .index
            .get_descriptor(change_keychain)
            .cloned()
            .expect("must have descriptor");

        let input_candidates = self.input_candidates().regroup(group_by_spk());

        let output = match self.index.index_of_spk(address.script_pubkey()) {
            // If we have the recipient address indexed, we want to include the
            // descriptor with the output so that it can be used to update the psbt.
            Some(&(keychain, index)) => {
                let desc = self
                    .index
                    .get_descriptor(keychain)
                    .expect("must have descriptor")
                    .at_derivation_index(index)?;

                Output::with_descriptor(desc, amount)
            }
            None => Output::with_script(address.script_pubkey(), amount),
        };

        let mut selector = Selector::new(
            &input_candidates,
            SelectorParams::new(
                feerate,
                vec![output],
                change_desc.at_derivation_index(next_index)?,
                ChangePolicyType::NoDustAndLeastWaste { longterm_feerate },
            ),
        )?;

        selector.select_with_algorithm(single_random_draw())?;

        let selection = selector.try_finalize().ok_or(anyhow::anyhow!("selection failed"))?;

        let params = PsbtParams {
            version: transaction::Version::TWO,
            fallback_locktime: absolute::LockTime::from_consensus(self.tip().height()),
            fallback_sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            mandate_full_tx_for_segwit_v0: false,
        };

        let psbt = selection.create_psbt(params)?;

        if selector.has_change().unwrap_or(false) {
            self.stage(index_changeset);
            self.index.mark_used(change_keychain, next_index);
        }

        Ok(psbt)
    }
}

/// Structures for updating the wallet
#[derive(Debug, Clone, Default)]
pub struct Update {
    pub tx_update: TxUpdate<ConfirmationBlockTime>,
    pub cp: Option<CheckPoint>,
    pub last_active_indices: BTreeMap<Keychain, u32>,
}

/// Create TxStatus from the given chain position (if confirmed).
fn status_from_position(pos: ChainPosition<ConfirmationBlockTime>) -> Option<TxStatus> {
    if let ChainPosition::Confirmed { anchor, .. } = pos {
        let conf_height = anchor.confirmation_height_upper_bound();
        let height =
            absolute::Height::from_consensus(conf_height).expect("must be valid block height");
        let time = absolute::Time::from_consensus(
            anchor
                .confirmation_time
                .try_into()
                .expect("confirmation time should fit into u32"),
        )
        .expect("must be valid block time");

        return Some(TxStatus { height, time });
    }
    None
}

/// Try to plan the output of `outpoint` with the available `assets`
fn try_plan(
    indexer: &KeychainTxOutIndex<Keychain>,
    outpoint: OutPoint,
    assets: &Assets,
) -> Option<Plan> {
    let ((keychain, index), _) = indexer.txout(outpoint)?;
    let desc = indexer
        .get_descriptor(keychain)?
        .at_derivation_index(index)
        .expect("must be valid derivation index");
    desc.plan(assets).ok()
}

/// Selection algorithm that sorts candidates randomly and selects until the target is met.
fn single_random_draw() -> impl FnMut(&mut Selector) -> Result<(), anyhow::Error> {
    let mut rng = rand::thread_rng();
    move |selector| {
        selector.inner_mut().sort_candidates_by(|_, _| {
            if rng.gen_bool(0.5) {
                cmp::Ordering::Greater
            } else {
                cmp::Ordering::Less
            }
        });
        selector.select_until_target_met()?;
        Ok(())
    }
}

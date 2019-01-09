// Copyright 2018 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::cmp;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::mem::size_of_val;

use ckey::{public_to_address, Public};
use ctypes::transaction::{Action, ParcelError};
use ctypes::BlockNumber;
use heapsize::HeapSizeOf;
use primitives::H256;
use rlp;
use table::Table;
use time::get_time;

use super::TransactionImportResult;
use crate::transaction::SignedTransaction;

/// Transaction with the same (sender, seq) can be replaced only if
/// `new_fee > old_fee + old_fee >> SHIFT`
const FEE_BUMP_SHIFT: usize = 3; // 2 = 25%, 3 = 12.5%, 4 = 6.25%

/// Point in time when transaction was inserted.
pub type PoolingInstant = BlockNumber;
const DEFAULT_POOLING_PERIOD: BlockNumber = 128;

/// Transaction origin
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxOrigin {
    /// Transaction coming from local RPC
    Local,
    /// External transaction received from network
    External,
    /// Transaction from retracted blocks
    RetractedBlock,
}

impl PartialOrd for TxOrigin {
    fn partial_cmp(&self, other: &TxOrigin) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TxOrigin {
    fn cmp(&self, other: &TxOrigin) -> Ordering {
        if *other == *self {
            return Ordering::Equal
        }

        match (*self, *other) {
            (TxOrigin::RetractedBlock, _) => Ordering::Less,
            (_, TxOrigin::RetractedBlock) => Ordering::Greater,
            (TxOrigin::Local, _) => Ordering::Less,
            _ => Ordering::Greater,
        }
    }
}

impl TxOrigin {
    fn is_local(self) -> bool {
        self == TxOrigin::Local
    }

    fn is_local_or_retracted(self) -> bool {
        self == TxOrigin::Local || self == TxOrigin::RetractedBlock
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TxTimelock {
    pub block: Option<BlockNumber>,
    pub timestamp: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
/// Light structure used to identify transaction and its order
struct TransactionOrder {
    /// Primary ordering factory. Difference between transaction seq and expected seq in state
    /// (e.g. Transaction(seq:5), State(seq:0) -> height: 5)
    /// High seq_height = Low priority (processed later)
    seq_height: u64,
    /// Fee of the transaction.
    fee: u64,
    /// Fee per bytes(rlp serialized) of the transaction
    fee_per_byte: u64,
    /// Heap usage of this transaction.
    mem_usage: usize,
    /// Hash to identify associated transaction
    hash: H256,
    /// Incremental id assigned when transaction is inserted to the pool.
    insertion_id: u64,
    /// Origin of the transaction
    origin: TxOrigin,
    /// Timelock
    timelock: TxTimelock,
}

impl TransactionOrder {
    fn for_transaction(item: &MemPoolItem, seq_seq: u64) -> Self {
        let rlp_bytes_len = rlp::encode(&item.tx).to_vec().len();
        let fee = item.tx.fee;
        let mem_usage = size_of_val(&item.tx) + item.tx.heap_size_of_children();
        ctrace!(MEM_POOL, "New tx with size {}", mem_usage);
        Self {
            seq_height: item.seq() - seq_seq,
            fee,
            mem_usage,
            fee_per_byte: fee / rlp_bytes_len as u64,
            hash: item.hash(),
            insertion_id: item.insertion_id,
            origin: item.origin,
            timelock: item.timelock,
        }
    }

    fn update_height(mut self, seq: u64, base_seq: u64) -> Self {
        self.seq_height = seq - base_seq;
        self
    }
}

impl Eq for TransactionOrder {}
impl PartialEq for TransactionOrder {
    fn eq(&self, other: &TransactionOrder) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl PartialOrd for TransactionOrder {
    fn partial_cmp(&self, other: &TransactionOrder) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TransactionOrder {
    fn cmp(&self, b: &TransactionOrder) -> Ordering {
        // Local transactions should always have priority
        if self.origin != b.origin {
            return self.origin.cmp(&b.origin)
        }

        // Check seq_height
        if self.seq_height != b.seq_height {
            return self.seq_height.cmp(&b.seq_height)
        }

        if self.fee_per_byte != b.fee_per_byte {
            return self.fee_per_byte.cmp(&b.fee_per_byte)
        }

        // Then compare fee
        if self.fee != b.fee {
            return b.fee.cmp(&self.fee)
        }

        // Compare timelock, prefer the nearer from the present.
        if self.timelock != b.timelock {
            return self.timelock.cmp(&b.timelock)
        }

        // Lastly compare insertion_id
        self.insertion_id.cmp(&b.insertion_id)
    }
}

/// Transaction item in the mem pool.
#[derive(Debug)]
struct MemPoolItem {
    /// Transaction.
    tx: SignedTransaction,
    /// Transaction origin.
    origin: TxOrigin,
    /// Insertion time
    insertion_time: PoolingInstant,
    /// ID assigned upon insertion, should be unique.
    insertion_id: u64,
    /// A timelock.
    timelock: TxTimelock,
}

impl MemPoolItem {
    fn new(
        tx: SignedTransaction,
        origin: TxOrigin,
        insertion_time: PoolingInstant,
        insertion_id: u64,
        timelock: TxTimelock,
    ) -> Self {
        MemPoolItem {
            tx,
            origin,
            insertion_time,
            insertion_id,
            timelock,
        }
    }

    fn hash(&self) -> H256 {
        self.tx.hash()
    }

    fn seq(&self) -> u64 {
        self.tx.seq
    }

    fn signer_public(&self) -> Public {
        self.tx.signer_public()
    }

    fn cost(&self) -> u64 {
        match &self.tx.action {
            Action::Pay {
                amount,
                ..
            } => self.tx.fee + *amount,
            Action::WrapCCC {
                amount,
                ..
            } => self.tx.fee + *amount,
            _ => self.tx.fee,
        }
    }
}

/// Holds transactions accessible by (signer_public, seq) and by priority
trait TransactionSet {
    /// Inserts `TransactionOrder` to this set. Transaction does not need to be unique -
    /// the same transaction may be validly inserted twice. Any previous transaction that
    /// it replaces (i.e. with the same `signer_public` and `seq`) should be returned.
    fn insert(&mut self, signer_public: Public, seq: u64, order: TransactionOrder) -> Option<TransactionOrder>;

    /// Remove low priority transactions if there is more than specified by given `limit`.
    ///
    /// It drops transactions from this set but also removes associated `VerifiedTransaction`.
    /// Returns public keys and lowest seqs of transactions removed because of limit.
    fn enforce_limit(&mut self, by_hash: &mut HashMap<H256, MemPoolItem>) -> Option<HashMap<Public, u64>>;

    /// Drop transaction from this set (remove from `by_priority` and `by_signer_public`)
    fn drop(&mut self, signer_public: &Public, seq: u64) -> Option<TransactionOrder>;

    /// Drop all transactions.
    fn clear(&mut self);

    /// Sets new limit for number of transactions in this `TransactionSet`.
    /// Note the limit is not applied (no transactions are removed) by calling this method.
    fn set_limit(&mut self, limit: usize);

    fn get_signer_public_row(&mut self, signer_public: &Public) -> Option<&HashMap<u64, TransactionOrder>>;
}

struct CurrentTxSet {
    by_priority: BTreeSet<TransactionOrder>,
    by_signer_public: Table<Public, u64, TransactionOrder>,
    by_fee: BTreeMap<u64, usize>,
    limit: usize,
    memory_limit: usize,
}

impl TransactionSet for CurrentTxSet {
    fn insert(&mut self, signer_public: Public, seq: u64, order: TransactionOrder) -> Option<TransactionOrder> {
        if !self.by_priority.insert(order) {
            return Some(order)
        }
        let order_fee = order.fee;
        let by_signer_public_replaced = self.by_signer_public.insert(signer_public, seq, order);
        *self.by_fee.entry(order_fee).or_insert(0) += 1;
        if let Some(ref old_order) = by_signer_public_replaced {
            assert!(
                self.by_priority.remove(old_order),
                "hash is in `by_signer_public`; all transactions in `by_signer_public` must be in `by_priority`; qed"
            );
            let delete_fee_entry = {
                let counter = self.by_fee.get_mut(&old_order.fee).expect(
                    "hash is in `by_signer_public`; all transactions' fee in `by_signer_public` must be in `by_fee`; qed",
                );
                *counter -= 1;
                *counter == 0
            };
            if delete_fee_entry {
                self.by_fee.remove(&old_order.fee);
            }
        }
        assert_eq!(self.by_priority.len(), self.by_signer_public.len());
        assert_eq!(self.by_fee.values().sum::<usize>(), self.by_signer_public.len());
        by_signer_public_replaced
    }

    fn enforce_limit(&mut self, by_hash: &mut HashMap<H256, MemPoolItem>) -> Option<HashMap<Public, u64>> {
        let mut count = 0;
        let mut mem_usage = 0;
        let to_drop: Vec<(Public, u64)> = {
            self.by_priority
                .iter()
                .filter(|order| {
                    // update transaction count and mem usage
                    count += 1;
                    mem_usage += order.mem_usage;

                    let is_own_or_retracted = order.origin.is_local() || order.origin == TxOrigin::RetractedBlock;
                    // Own and retracted transactions are allowed to go above all limits.
                    !is_own_or_retracted && (mem_usage > self.memory_limit || count > self.limit)
                })
                .map(|order| {
                    by_hash.get(&order.hash).expect(
                        "All transactions in `self.by_priority` and `self.by_signer_public` are kept in sync with `by_hash`.",
                    )
                })
                .map(|tx| (tx.signer_public(), tx.seq()))
                .collect()
        };

        Some(to_drop.into_iter().fold(HashMap::new(), |mut removed, (sender, seq)| {
            let order = self
                .drop(&sender, seq)
                .expect("Transaction has just been found in `by_priority`; so it is in `by_signer_public` also.");
            ctrace!(MEM_POOL, "Dropped out of limit transaction: {:?}", order.hash);

            by_hash
                .remove(&order.hash)
                .expect("hash is in `by_priorty`; all hashes in `by_priority` must be in `by_hash`; qed");

            let min = removed.get(&sender).map_or(seq, |val| cmp::min(*val, seq));
            removed.insert(sender, min);
            removed
        }))
    }

    fn drop(&mut self, signer_public: &Public, seq: u64) -> Option<TransactionOrder> {
        if let Some(tx_order) = self.by_signer_public.remove(signer_public, &seq) {
            let delete_fee_entry = {
                let counter = self.by_fee.get_mut(&tx_order.fee).expect(
                    "hash is in `by_signer_public`; all transactions' fee in `by_signer_public` must be in `by_fee`; qed",
                );
                *counter -= 1;
                *counter == 0
            };
            if delete_fee_entry {
                self.by_fee.remove(&tx_order.fee);
            }
            assert!(
                self.by_priority.remove(&tx_order),
                "hash is in `by_signer_public`; all transactions in `by_signer_public` must be in `by_priority`; qed"
            );
            assert_eq!(self.by_priority.len(), self.by_signer_public.len());
            assert_eq!(self.by_fee.values().sum::<usize>(), self.by_signer_public.len());
            return Some(tx_order)
        }
        assert_eq!(self.by_priority.len(), self.by_signer_public.len());
        assert_eq!(self.by_fee.values().sum::<usize>(), self.by_signer_public.len());
        None
    }

    fn clear(&mut self) {
        self.by_priority.clear();
        self.by_signer_public.clear();
    }

    fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }

    fn get_signer_public_row(&mut self, signer_public: &Public) -> Option<&HashMap<u64, TransactionOrder>> {
        self.by_signer_public.row(signer_public)
    }
}

impl CurrentTxSet {
    /// Get the minimum fee that we can accept into this pool that wouldn't cause the transaction to
    /// immediately be dropped. 0 if the pool isn't at capacity; 1 plus the lowest if it is.
    fn fee_entry_limit(&self) -> u64 {
        match self.by_fee.keys().next() {
            Some(k) if self.by_priority.len() >= self.limit => k + 1,
            _ => 0,
        }
    }
}

struct FutureTxSet {
    by_priority: BTreeSet<TransactionOrder>,
    by_signer_public: Table<Public, u64, TransactionOrder>,
    limit: usize,
    memory_limit: usize,
}

impl TransactionSet for FutureTxSet {
    fn insert(&mut self, signer_public: Public, seq: u64, order: TransactionOrder) -> Option<TransactionOrder> {
        if !self.by_priority.insert(order) {
            return Some(order)
        }
        let by_signer_public_replaced = self.by_signer_public.insert(signer_public, seq, order);
        if let Some(ref old_order) = by_signer_public_replaced {
            assert!(
                self.by_priority.remove(old_order),
                "hash is in `by_signer_public`; all transactions in `by_signer_public` must be in `by_priority`; qed"
            );
        }
        assert_eq!(self.by_priority.len(), self.by_signer_public.len());
        by_signer_public_replaced
    }

    fn enforce_limit(&mut self, by_hash: &mut HashMap<H256, MemPoolItem>) -> Option<HashMap<Public, u64>> {
        let mut count = 0;
        let mut mem_usage = 0;
        let to_drop: Vec<(Public, u64)> = {
            self.by_priority
                .iter()
                .filter(|order| {
                    // update transaction count and mem usage
                    count += 1;
                    mem_usage += order.mem_usage;

                    // Own and retracted transactions are allowed to go above all limits.
                    !order.origin.is_local_or_retracted() && (mem_usage > self.memory_limit || count > self.limit)
                })
                .map(|order| {
                    by_hash.get(&order.hash).expect(
                        "All transactions in `self.by_priority` and `self.by_signer_public` are kept in sync with `by_hash`.",
                    )
                })
                .map(|tx| (tx.signer_public(), tx.seq()))
                .collect()
        };

        Some(to_drop.into_iter().fold(HashMap::new(), |mut removed, (sender, seq)| {
            let order = self
                .drop(&sender, seq)
                .expect("Transaction has just been found in `by_priority`; so it is in `by_signer_public` also.");
            ctrace!(MEM_POOL, "Dropped out of limit transaction: {:?}", order.hash);

            by_hash
                .remove(&order.hash)
                .expect("hash is in `by_priorty`; all hashes in `by_priority` must be in `by_hash`; qed");

            let min = removed.get(&sender).map_or(seq, |val| cmp::min(*val, seq));
            removed.insert(sender, min);
            removed
        }))
    }

    fn drop(&mut self, signer_public: &Public, seq: u64) -> Option<TransactionOrder> {
        if let Some(tx_order) = self.by_signer_public.remove(signer_public, &seq) {
            assert!(
                self.by_priority.remove(&tx_order),
                "hash is in `by_signer_public`; all transactions in `by_signer_public` must be in `by_priority`; qed"
            );
            assert_eq!(self.by_priority.len(), self.by_signer_public.len());
            return Some(tx_order)
        }
        assert_eq!(self.by_priority.len(), self.by_signer_public.len());
        None
    }

    fn clear(&mut self) {
        self.by_priority.clear();
        self.by_signer_public.clear();
    }

    fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }

    fn get_signer_public_row(&mut self, signer_public: &Public) -> Option<&HashMap<u64, TransactionOrder>> {
        self.by_signer_public.row(signer_public)
    }
}

impl FutureTxSet {
    /// Update the heights of the transaction orders as the given input.
    ///
    /// If there are transactions which are older than `base_seq`, the function removes them from the set.
    fn update_base_seq(&mut self, by_hash: &mut HashMap<H256, MemPoolItem>, signer_public: &Public, base_seq: u64) {
        let row = match self.by_signer_public.row_mut(signer_public) {
            Some(row) => row,
            None => return,
        };

        for (seq, order) in row.iter_mut() {
            assert!(
                self.by_priority.remove(&order),
                "hash is in `by_signer_public`; all transactions in `by_signer_public` must be in `by_priority`; qed"
            );
            if *seq < base_seq {
                ctrace!(MEM_POOL, "Removing old tx: {:?} (seq: {} < {})", order.hash, seq, base_seq);
                by_hash.remove(&order.hash).expect("All transactions in `future` are also in `by_hash`");
            } else {
                let new_order = order.update_height(*seq, base_seq);
                *order = new_order;
                self.by_priority.insert(new_order);
            }
        }

        row.retain(|seq, _| *seq >= base_seq);
    }
}

pub struct MemPool {
    /// Fee threshold for transactions that can be imported to this pool (defaults to 0)
    minimal_fee: u64,
    /// Maximal time transaction may occupy the pool.
    /// When we reach `max_time_in_pool / 2^3` we re-validate
    /// account balance.
    max_time_in_pool: PoolingInstant,
    /// Priority queue for transactions that can go to block
    current: CurrentTxSet,
    /// Priority queue for transactions that has been received but are not yet valid to go to block
    future: FutureTxSet,
    /// All transactions managed by pool indexed by hash
    by_hash: HashMap<H256, MemPoolItem>,
    /// Last seq of transaction in current (to quickly check next expected transaction)
    last_seqs: HashMap<Public, u64>,
    /// Next id that should be assigned to a transaction imported to the pool.
    next_transaction_id: u64,
}

impl Default for MemPool {
    fn default() -> Self {
        MemPool::new()
    }
}

impl MemPool {
    /// Creates new instance of this Queue
    pub fn new() -> Self {
        Self::with_limits(8192, usize::max_value())
    }

    /// Create new instance of this Queue with specified limits
    pub fn with_limits(limit: usize, memory_limit: usize) -> Self {
        let current = CurrentTxSet {
            by_priority: BTreeSet::new(),
            by_signer_public: Table::new(),
            by_fee: BTreeMap::default(),
            limit,
            memory_limit,
        };

        let future = FutureTxSet {
            by_priority: BTreeSet::new(),
            by_signer_public: Table::new(),
            limit,
            memory_limit,
        };

        MemPool {
            minimal_fee: 0,
            max_time_in_pool: DEFAULT_POOLING_PERIOD,
            current,
            future,
            by_hash: HashMap::new(),
            last_seqs: HashMap::new(),
            next_transaction_id: 0,
        }
    }

    /// Set the new limit for `current` and `future` queue.
    pub fn set_limit(&mut self, limit: usize) {
        self.current.set_limit(limit);
        self.future.set_limit(limit);
        // And ensure the limits
        self.current.enforce_limit(&mut self.by_hash);
        self.future.enforce_limit(&mut self.by_hash);
    }

    /// Returns current limit of transactions in the pool.
    pub fn limit(&self) -> usize {
        self.current.limit
    }

    /// Get the minimal fee.
    pub fn minimal_fee(&self) -> u64 {
        self.minimal_fee
    }

    /// Sets new fee threshold for incoming transactions.
    /// Any transaction already imported to the pool is not affected.
    pub fn set_minimal_fee(&mut self, min_fee: u64) {
        self.minimal_fee = min_fee;
    }

    /// Get one more than the lowest fee in the pool iff the pool is
    /// full, otherwise 0.
    pub fn effective_minimum_fee(&self) -> u64 {
        self.current.fee_entry_limit()
    }

    /// Returns current status for this pool
    pub fn status(&self) -> MemPoolStatus {
        MemPoolStatus {
            pending: self.current.by_priority.len(),
            future: self.future.by_priority.len(),
        }
    }

    /// Add signed transaction to pool to be verified and imported.
    ///
    /// NOTE details_provider methods should be cheap to compute
    /// otherwise it might open up an attack vector.
    pub fn add<F>(
        &mut self,
        tx: SignedTransaction,
        origin: TxOrigin,
        time: PoolingInstant,
        timestamp: u64,
        timelock: TxTimelock,
        fetch_account: &F,
    ) -> Result<TransactionImportResult, ParcelError>
    where
        F: Fn(&Public) -> AccountDetails, {
        let client_account = fetch_account(&tx.signer_public());
        self.verify_transaction(&tx, origin, &client_account)?;

        // No invalid transactions beyond this point.
        let id = self.next_transaction_id;
        self.next_transaction_id += 1;
        let item = MemPoolItem::new(tx, origin, time, id, timelock);
        let result = self.import_transaction(item, client_account.seq, timestamp);
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
        result
    }

    /// Checks the current seq for all transactions' senders in the pool and removes the old transactions.
    pub fn remove_old<F>(&mut self, fetch_account: &F, current_time: PoolingInstant, timestamp: u64)
    where
        F: Fn(&Public) -> AccountDetails, {
        let signers = self
            .current
            .by_signer_public
            .keys()
            .chain(self.future.by_signer_public.keys())
            .map(|sender| (*sender, fetch_account(sender)))
            .collect::<HashMap<_, _>>();

        for (signer, details) in signers.iter() {
            self.cull(*signer, details.seq, current_time, timestamp);
        }

        let max_time = self.max_time_in_pool;
        let balance_check = max_time >> 3;
        // Clear transactions occupying the pool too long
        let invalid = self
            .by_hash
            .iter()
            .filter(|&(_, ref tx)| !tx.origin.is_local())
            .map(|(hash, tx)| (hash, tx, current_time.saturating_sub(tx.insertion_time)))
            .filter_map(|(hash, tx, time_diff)| {
                if time_diff > max_time {
                    return Some(*hash)
                }

                if time_diff > balance_check {
                    return match signers.get(&tx.signer_public()) {
                        Some(details) if tx.cost() > details.balance => Some(*hash),
                        _ => None,
                    }
                }

                None
            })
            .collect::<Vec<_>>();
        let fetch_seq =
            |a: &Public| signers.get(a).expect("We fetch details for all signers from both current and future").seq;
        for hash in invalid {
            self.remove(&hash, &fetch_seq, current_time, timestamp);
        }
    }

    /// Removes invalid transaction identified by hash from pool.
    /// Assumption is that this transaction seq is not related to client seq,
    /// so transactions left in pool are processed according to client seq.
    ///
    /// If gap is introduced marks subsequent transactions as future
    pub fn remove<F>(&mut self, tx_hash: &H256, fetch_seq: &F, current_time: PoolingInstant, timestamp: u64)
    where
        F: Fn(&Public) -> u64, {
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
        let tx = if let Some(tx) = self.by_hash.remove(tx_hash) {
            tx
        } else {
            // We don't know this transaction
            return
        };

        let signer_public = tx.signer_public();
        let seq = tx.seq();
        let current_seq = fetch_seq(&signer_public);

        ctrace!(MEM_POOL, "Removing invalid transaction: {:?}", tx.hash());

        // Remove from future
        let order = self.future.drop(&signer_public, seq);
        if order.is_some() {
            self.update_future(&signer_public, current_seq);
            // And now lets check if there is some chain of transactions in future
            // that should be placed in current
            self.move_matching_future_to_current(signer_public, current_seq, current_seq, current_time, timestamp);
            assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
            return
        }

        // Remove from current
        let order = self.current.drop(&signer_public, seq);
        if order.is_some() {
            // This will keep consistency in pool
            // Moves all to future and then promotes a batch from current:
            self.cull_internal(signer_public, current_seq, current_time, timestamp);
            assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
            return
        }
    }

    /// Removes all transactions from particular signer up to (excluding) given client (state) seq.
    /// Client (State) seq = next valid seq for this signer.
    pub fn cull(&mut self, signer_public: Public, client_seq: u64, current_time: PoolingInstant, timestamp: u64) {
        // Check if there is anything in current...
        let should_check_in_current = self.current.by_signer_public.row(&signer_public)
            // If seq == client_seq nothing is changed
            .and_then(|by_seq| by_seq.keys().find(|seq| **seq < client_seq))
            .map(|_| ());
        // ... or future
        let should_check_in_future = self.future.by_signer_public.row(&signer_public)
            // if seq == client_seq we need to promote to current
            .and_then(|by_seq| by_seq.keys().find(|seq| **seq <= client_seq))
            .map(|_| ());

        if should_check_in_current.or(should_check_in_future).is_none() {
            return
        }

        self.cull_internal(signer_public, client_seq, current_time, timestamp);
    }

    /// Removes all elements (in any state) from the pool
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.current.clear();
        self.future.clear();
        self.by_hash.clear();
        self.last_seqs.clear();
    }

    /// Finds teansaction in the pool by hash (if any)
    #[allow(dead_code)]
    pub fn find(&self, hash: &H256) -> Option<SignedTransaction> {
        self.by_hash.get(hash).map(|tx| tx.tx.clone())
    }

    /// Returns highest transaction seq for given signer.
    #[allow(dead_code)]
    pub fn last_seq(&self, signer_public: &Public) -> Option<u64> {
        self.last_seqs.get(signer_public).cloned()
    }

    /// Returns top transactions from the pool ordered by priority.
    pub fn top_transactions(&self, size_limit: usize) -> Vec<SignedTransaction> {
        let mut current_size: usize = 0;
        self.current
            .by_priority
            .iter()
            .map(|t| {
                self.by_hash
                    .get(&t.hash)
                    .expect("All transactions in `current` and `future` are always included in `by_hash`")
            })
            .take_while(|t| {
                let encoded_byte_array: Vec<u8> = rlp::encode(&t.tx).into_vec();
                let size_in_byte = encoded_byte_array.len();
                current_size += size_in_byte;
                current_size < size_limit
            })
            .map(|t| t.tx.clone())
            .collect()
    }

    /// Return all future transactions.
    pub fn future_tranasctions(&self) -> Vec<SignedTransaction> {
        self.future
            .by_priority
            .iter()
            .map(|t| {
                self.by_hash
                    .get(&t.hash)
                    .expect("All transactions in `current` and `future` are always included in `by_hash`")
            })
            .map(|t| t.tx.clone())
            .collect()
    }

    /// Returns true if there is at least one local transaction pending
    pub fn has_local_pending_transactions(&self) -> bool {
        self.current.by_priority.iter().any(|tx| tx.origin == TxOrigin::Local)
    }

    /// Verify signed transaction about its fee, balance of its fee payer, and its signature.
    fn verify_transaction(
        &self,
        tx: &SignedTransaction,
        origin: TxOrigin,
        client_account: &AccountDetails,
    ) -> Result<(), ParcelError> {
        if origin != TxOrigin::Local && tx.fee < self.minimal_fee {
            ctrace!(
                MEM_POOL,
                "Dropping transaction below minimal fee: {:?} (gp: {} < {})",
                tx.hash(),
                tx.fee,
                self.minimal_fee
            );

            return Err(ParcelError::InsufficientFee {
                minimal: self.minimal_fee,
                got: tx.fee,
            })
        }

        let full_pools_lowest = self.effective_minimum_fee();
        if tx.fee < full_pools_lowest && origin != TxOrigin::Local {
            ctrace!(
                MEM_POOL,
                "Dropping transaction below lowest fee in a full pool: {:?} (gp: {} < {})",
                tx.hash(),
                tx.fee,
                full_pools_lowest
            );

            return Err(ParcelError::InsufficientFee {
                minimal: full_pools_lowest,
                got: tx.fee,
            })
        }

        if client_account.balance < tx.fee {
            ctrace!(
                MEM_POOL,
                "Dropping transaction without sufficient balance: {:?} ({} < {})",
                tx.hash(),
                client_account.balance,
                tx.fee
            );

            return Err(ParcelError::InsufficientBalance {
                address: public_to_address(&tx.signer_public()),
                cost: tx.fee,
                balance: client_account.balance,
            })
        }

        tx.check_low_s()?;

        Ok(())
    }

    /// Adds VerifiedTransaction to this pool.
    ///
    /// Determines if it should be placed in current or future. When transaction is
    /// imported to `current` also checks if there are any `future` transactions that should be promoted because of
    /// this.
    ///
    /// It ignores transactions that has already been imported (same `hash`) and replaces the transaction
    /// iff `(address, seq)` is the same but `fee` is higher.
    ///
    /// Returns `true` when transaction was imported successfully
    fn import_transaction(
        &mut self,
        tx: MemPoolItem,
        state_seq: u64,
        timestamp: u64,
    ) -> Result<TransactionImportResult, ParcelError> {
        if self.by_hash.get(&tx.hash()).is_some() {
            // Transaction is already imported.
            ctrace!(MEM_POOL, "Dropping already imported transaction: {:?}", tx.hash());
            return Err(ParcelError::TransactionAlreadyImported)
        }

        let signer_public = tx.signer_public();
        let seq = tx.seq();
        let hash = tx.hash();

        // The transaction might be old, let's check that.
        // This has to be the first test, otherwise calculating
        // seq height would result in overflow.
        if seq < state_seq {
            // Droping transaction
            ctrace!(MEM_POOL, "Dropping old transaction: {:?} (seq: {} < {})", tx.hash(), seq, state_seq);
            return Err(ParcelError::Old)
        }

        // Update seqs of transactions in future (remove old transactions)
        self.update_future(&signer_public, state_seq);
        // State seq could be updated. Maybe there are some more items waiting in future?
        self.move_matching_future_to_current(signer_public, state_seq, state_seq, tx.insertion_time, timestamp);
        // Check the next expected seq (might be updated by move above)
        let next_seq = self.last_seqs.get(&signer_public).map_or(state_seq, |n| *n + 1);

        if tx.origin.is_local() {
            self.mark_transactions_local(&signer_public);
        }

        // Future transaction
        if seq > next_seq {
            // We have a gap - put to future.
            // Insert transaction (or replace old one with lower fee)
            check_too_cheap(Self::replace_transaction(tx, state_seq, &mut self.future, &mut self.by_hash))?;
            // Enforce limit in Future
            let removed = self.future.enforce_limit(&mut self.by_hash);
            // Return an error if this transaction was not imported because of limit.
            check_if_removed(&signer_public, seq, removed)?;

            cdebug!(MEM_POOL, "Importing transaction to future: {:?}", hash);
            cdebug!(MEM_POOL, "status: {:?}", self.status());
            return Ok(TransactionImportResult::Future)
        }

        // We might have filled a gap - move some more transactions from future
        self.move_matching_future_to_current(signer_public, seq, state_seq, tx.insertion_time, timestamp);
        self.move_matching_future_to_current(signer_public, seq + 1, state_seq, tx.insertion_time, timestamp);

        if Self::should_wait_timelock(&tx.timelock, tx.insertion_time, timestamp) {
            // Check same seq is in current. If it
            // is than move the following current items to future.
            let best_block_number = tx.insertion_time;
            let moved_to_future_flag = self.current.by_signer_public.get(&signer_public, &seq).is_some();
            if moved_to_future_flag {
                self.move_all_to_future(&signer_public, state_seq);
            }

            check_too_cheap(Self::replace_transaction(tx, state_seq, &mut self.future, &mut self.by_hash))?;

            if moved_to_future_flag {
                self.move_matching_future_to_current(signer_public, state_seq, state_seq, best_block_number, timestamp);
            }

            let removed = self.future.enforce_limit(&mut self.by_hash);
            check_if_removed(&signer_public, seq, removed)?;
            cdebug!(MEM_POOL, "Imported transaction to future: {:?}", hash);
            cdebug!(MEM_POOL, "status: {:?}", self.status());
            return Ok(TransactionImportResult::Future)
        }

        // Replace transaction if any
        check_too_cheap(Self::replace_transaction(tx, state_seq, &mut self.current, &mut self.by_hash))?;
        // Keep track of highest seq stored in current
        let new_max = self.last_seqs.get(&signer_public).map_or(seq, |n| cmp::max(seq, *n));
        self.last_seqs.insert(signer_public, new_max);

        // Also enforce the limit
        let removed = self.current.enforce_limit(&mut self.by_hash);
        // If some transaction were removed because of limit we need to update last_seqs also.
        self.update_last_seqs(&removed);
        // Trigger error if the transaction we are importing was removed.
        check_if_removed(&signer_public, seq, removed)?;

        cdebug!(MEM_POOL, "Imported transaction to current: {:?}", hash);
        cdebug!(MEM_POOL, "status: {:?}", self.status());
        Ok(TransactionImportResult::Current)
    }

    fn should_wait_timelock(timelock: &TxTimelock, best_block_number: BlockNumber, best_block_timestamp: u64) -> bool {
        if let Some(block_number) = timelock.block {
            if block_number > best_block_number + 1 {
                return true
            }
        }
        if let Some(timestamp) = timelock.timestamp {
            if timestamp > cmp::max(get_time().sec as u64, best_block_timestamp + 1) {
                return true
            }
        }
        false
    }

    /// Always updates future and moves transaction from current to future.
    fn cull_internal(&mut self, sender: Public, client_seq: u64, current_time: PoolingInstant, timestamp: u64) {
        // We will either move transaction to future or remove it completely
        // so there will be no transactions from this sender in current
        self.last_seqs.remove(&sender);
        // First update height of transactions in future to avoid collisions
        self.update_future(&sender, client_seq);
        // This should move all current transactions to future and remove old transactions
        self.move_all_to_future(&sender, client_seq);
        // And now lets check if there is some batch of transactions in future
        // that should be placed in current. It should also update last_seqs.
        self.move_matching_future_to_current(sender, client_seq, client_seq, current_time, timestamp);
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
    }

    fn update_last_seqs(&mut self, removed_min_seqs: &Option<HashMap<Public, u64>>) {
        if let Some(ref min_seqs) = *removed_min_seqs {
            for (sender, seq) in min_seqs.iter() {
                if seq == &0 {
                    self.last_seqs.remove(sender);
                } else {
                    self.last_seqs.insert(*sender, *seq - 1);
                }
            }
        }
    }

    /// Update height of all transactions in future transactions set.
    fn update_future(&mut self, signer_public: &Public, current_seq: u64) {
        self.future.update_base_seq(&mut self.by_hash, signer_public, current_seq);
    }

    /// Checks if there are any transactions in `future` that should actually be promoted to `current`
    /// (because seq matches).
    fn move_matching_future_to_current(
        &mut self,
        public: Public,
        mut current_seq: u64,
        first_seq: u64,
        best_block_number: BlockNumber,
        best_block_timestamp: u64,
    ) {
        let mut update_last_seq_to = None;
        {
            let by_seq = if let Some(by_seq) = self.future.by_signer_public.row_mut(&public) {
                by_seq
            } else {
                return
            };
            while let Some(order) = by_seq.get(&current_seq).cloned() {
                if Self::should_wait_timelock(&order.timelock, best_block_number, best_block_timestamp) {
                    break
                }
                let order = by_seq.remove(&current_seq).expect("None is tested in the while condition above.");
                self.future.by_priority.remove(&order);
                // Put to current
                let order = order.update_height(current_seq, first_seq);
                if let Some(old) = self.current.insert(public, current_seq, order) {
                    Self::replace_orders(public, current_seq, old, order, &mut self.current, &mut self.by_hash);
                }
                update_last_seq_to = Some(current_seq);
                current_seq += 1;
            }
        }
        self.future.by_signer_public.clear_if_empty(&public);
        if let Some(x) = update_last_seq_to {
            // Update last inserted seq
            self.last_seqs.insert(public, x);
        }
    }

    /// Drop all transactions from given signer from `current`.
    /// Either moves them to `future` or removes them from pool completely.
    fn move_all_to_future(&mut self, signer_public: &Public, current_seq: u64) {
        let all_seqs_from_sender = match self.current.by_signer_public.row(signer_public) {
            Some(row_map) => row_map.keys().cloned().collect::<Vec<_>>(),
            None => vec![],
        };

        for k in all_seqs_from_sender {
            // Goes to future or is removed
            let order = self
                .current
                .drop(signer_public, k)
                .expect("iterating over a collection that has been retrieved above; qed");
            if k >= current_seq {
                let order = order.update_height(k, current_seq);
                if let Some(old) = self.future.insert(*signer_public, k, order) {
                    Self::replace_orders(*signer_public, k, old, order, &mut self.future, &mut self.by_hash);
                }
            } else {
                ctrace!(MEM_POOL, "Removing old transaction: {:?} (seq: {} < {})", order.hash, k, current_seq);
                self.by_hash.remove(&order.hash).expect("All transactions in `future` are also in `by_hash`");
            }
        }
        self.future.enforce_limit(&mut self.by_hash);
    }

    /// Marks all transactions from particular sender as local transactions
    fn mark_transactions_local(&mut self, signer: &Public) {
        fn mark_local(signer_public: &Public, set: &mut TransactionSet) {
            // Mark all transactions from this signer as local
            let seqs_from_sender = set
                .get_signer_public_row(signer_public)
                .map(|row_map| {
                    row_map
                        .iter()
                        .filter_map(|(seq, order)| {
                            if order.origin.is_local() {
                                None
                            } else {
                                Some(*seq)
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(Vec::new);

            for k in seqs_from_sender {
                let mut order =
                    set.drop(signer_public, k).expect("transaction known to be in self.current/self.future; qed");
                order.origin = TxOrigin::Local;
                set.insert(*signer_public, k, order);
            }
        }

        mark_local(signer, &mut self.current);
        mark_local(signer, &mut self.future);
    }

    /// Replaces transaction in given set (could be `future` or `current`).
    ///
    /// If there is already transaction with same `(sender, seq)` it will be replaced iff `fee` is higher.
    /// One of the transaction is dropped from set and also removed from pool entirely (from `by_hash`).
    ///
    /// Returns `true` if transaction actually got to the pool (`false` if there was already a transaction with higher
    /// fee)
    fn replace_transaction(
        tx: MemPoolItem,
        base_seq: u64,
        set: &mut TransactionSet,
        by_hash: &mut HashMap<H256, MemPoolItem>,
    ) -> bool {
        let order = TransactionOrder::for_transaction(&tx, base_seq);
        let hash = tx.hash();
        let signer_public = tx.signer_public();
        let seq = tx.seq();

        let old_hash = by_hash.insert(hash, tx);
        assert!(old_hash.is_none(), "Each hash has to be inserted exactly once.");

        ctrace!(MEM_POOL, "Inserting: {:?}", order);

        if let Some(old) = set.insert(signer_public, seq, order) {
            Self::replace_orders(signer_public, seq, old, order, set, by_hash)
        } else {
            true
        }
    }

    fn replace_orders(
        signer_public: Public,
        seq: u64,
        old: TransactionOrder,
        order: TransactionOrder,
        set: &mut TransactionSet,
        by_hash: &mut HashMap<H256, MemPoolItem>,
    ) -> bool {
        // There was already transaction in pool. Let's check which one should stay
        let old_fee = old.fee;
        let new_fee = order.fee;
        let min_required_fee = old_fee + (old_fee >> FEE_BUMP_SHIFT);

        if min_required_fee > new_fee {
            ctrace!(
                MEM_POOL,
                "Didn't insert transaction because fee was too low: {:?} ({:?} stays in the pool)",
                order.hash,
                old.hash
            );
            // Put back old transaction since it has greater priority (higher fee)
            set.insert(signer_public, seq, old);
            // and remove new one
            by_hash
                .remove(&order.hash)
                .expect("The hash has been just inserted and no other line is altering `by_hash`.");
            false
        } else {
            ctrace!(
                MEM_POOL,
                "Replaced transaction: {:?} with transaction with higher fee: {:?}",
                old.hash,
                order.hash
            );
            // Make sure we remove old transaction entirely
            by_hash.remove(&old.hash).expect("The hash is coming from `future` so it has to be in `by_hash`.");
            true
        }
    }
}

#[derive(Debug)]
/// Current status of the pool
pub struct MemPoolStatus {
    /// Number of pending transactions (ready to go to block)
    pub pending: usize,
    /// Number of future transactions (waiting for transactions with lower seqs first)
    pub future: usize,
}

#[derive(Debug)]
/// Details of account
pub struct AccountDetails {
    /// Most recent account seq
    pub seq: u64,
    /// Current account balance
    pub balance: u64,
}

fn check_too_cheap(is_in: bool) -> Result<(), ParcelError> {
    if is_in {
        Ok(())
    } else {
        Err(ParcelError::TooCheapToReplace)
    }
}

fn check_if_removed(sender: &Public, seq: u64, dropped: Option<HashMap<Public, u64>>) -> Result<(), ParcelError> {
    match dropped {
        Some(dropped) => match dropped.get(sender) {
            Some(min) if seq >= *min => Err(ParcelError::LimitReached),
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

#[cfg(test)]
pub mod test {
    use std::cmp::Ordering;

    use ckey::{Generator, Random};
    use ctypes::transaction::{AssetMintOutput, Transaction};
    use primitives::H160;

    use super::*;

    #[test]
    fn origin_ordering() {
        assert_eq!(TxOrigin::Local.cmp(&TxOrigin::External), Ordering::Less);
        assert_eq!(TxOrigin::RetractedBlock.cmp(&TxOrigin::Local), Ordering::Less);
        assert_eq!(TxOrigin::RetractedBlock.cmp(&TxOrigin::External), Ordering::Less);

        assert_eq!(TxOrigin::External.cmp(&TxOrigin::Local), Ordering::Greater);
        assert_eq!(TxOrigin::Local.cmp(&TxOrigin::RetractedBlock), Ordering::Greater);
        assert_eq!(TxOrigin::External.cmp(&TxOrigin::RetractedBlock), Ordering::Greater);
    }

    #[test]
    fn timelock_ordering() {
        assert_eq!(
            TxTimelock {
                block: None,
                timestamp: None
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: None
            }),
            Ordering::Less
        );
        assert_eq!(
            TxTimelock {
                block: None,
                timestamp: None
            }
            .cmp(&TxTimelock {
                block: None,
                timestamp: Some(100)
            }),
            Ordering::Less
        );

        // Block is the prior condition.
        assert_eq!(
            TxTimelock {
                block: Some(9),
                timestamp: None
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: None
            }),
            Ordering::Less
        );
        assert_eq!(
            TxTimelock {
                block: Some(9),
                timestamp: None
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            TxTimelock {
                block: Some(9),
                timestamp: Some(100)
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: None
            }),
            Ordering::Less
        );
        assert_eq!(
            TxTimelock {
                block: Some(9),
                timestamp: Some(99)
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            TxTimelock {
                block: Some(9),
                timestamp: Some(101)
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            TxTimelock {
                block: Some(11),
                timestamp: None
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: None
            }),
            Ordering::Greater
        );
        assert_eq!(
            TxTimelock {
                block: Some(11),
                timestamp: None
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Greater
        );
        assert_eq!(
            TxTimelock {
                block: Some(11),
                timestamp: Some(100)
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: None
            }),
            Ordering::Greater
        );
        assert_eq!(
            TxTimelock {
                block: Some(11),
                timestamp: Some(99)
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Greater
        );
        assert_eq!(
            TxTimelock {
                block: Some(11),
                timestamp: Some(101)
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Greater
        );

        // Compare timestamp if blocks are equal.
        assert_eq!(
            TxTimelock {
                block: Some(10),
                timestamp: None
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            TxTimelock {
                block: Some(10),
                timestamp: Some(99)
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Equal
        );
        assert_eq!(
            TxTimelock {
                block: Some(10),
                timestamp: Some(101)
            }
            .cmp(&TxTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Greater
        );
        assert_eq!(
            TxTimelock {
                block: None,
                timestamp: None
            }
            .cmp(&TxTimelock {
                block: None,
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            TxTimelock {
                block: None,
                timestamp: Some(99)
            }
            .cmp(&TxTimelock {
                block: None,
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            TxTimelock {
                block: None,
                timestamp: Some(100)
            }
            .cmp(&TxTimelock {
                block: None,
                timestamp: Some(100)
            }),
            Ordering::Equal
        );
        assert_eq!(
            TxTimelock {
                block: None,
                timestamp: Some(101)
            }
            .cmp(&TxTimelock {
                block: None,
                timestamp: Some(100)
            }),
            Ordering::Greater
        );
    }

    #[test]
    fn mint_transaction_does_not_increase_cost() {
        let shard_id = 0xCCC;

        let fee = 100;
        let tx = Transaction {
            seq: 0,
            fee,
            network_id: "tc".into(),
            action: Action::MintAsset {
                network_id: "tc".into(),
                shard_id,
                metadata: "Metadata".to_string(),
                output: Box::new(AssetMintOutput {
                    lock_script_hash: H160::zero(),
                    parameters: vec![],
                    amount: None,
                }),
                approver: None,
                administrator: None,
                allowed_script_hashes: vec![],
                approvals: vec![],
            },
        };
        let timelock = TxTimelock {
            block: None,
            timestamp: None,
        };
        let keypair = Random.generate().unwrap();
        let signed = SignedTransaction::new_with_sign(tx, keypair.private());
        let item = MemPoolItem::new(signed, TxOrigin::Local, 0, 0, timelock);

        assert_eq!(fee, item.cost());
    }

    #[test]
    fn transfer_transaction_does_not_increase_cost() {
        let fee = 100;
        let tx = Transaction {
            seq: 0,
            fee,
            network_id: "tc".into(),
            action: Action::TransferAsset {
                network_id: "tc".into(),
                burns: vec![],
                inputs: vec![],
                outputs: vec![],
                orders: vec![],
                approvals: vec![],
            },
        };
        let timelock = TxTimelock {
            block: None,
            timestamp: None,
        };
        let keypair = Random.generate().unwrap();
        let signed = SignedTransaction::new_with_sign(tx, keypair.private());
        let item = MemPoolItem::new(signed, TxOrigin::Local, 0, 0, timelock);

        assert_eq!(fee, item.cost());
    }

    #[test]
    fn pay_transaction_increases_cost() {
        let fee = 100;
        let amount = 100_000;
        let receiver = 1u64.into();
        let keypair = Random.generate().unwrap();
        let tx = Transaction {
            seq: 0,
            fee,
            network_id: "tc".into(),
            action: Action::Pay {
                receiver,
                amount,
            },
        };
        let timelock = TxTimelock {
            block: None,
            timestamp: None,
        };
        let signed = SignedTransaction::new_with_sign(tx, keypair.private());
        let item = MemPoolItem::new(signed, TxOrigin::Local, 0, 0, timelock);

        assert_eq!(fee + amount, item.cost());
    }

    #[test]
    fn fee_per_byte_order_simple() {
        let order1 = create_transaction_order(1_000_000_000, 100);
        let order2 = create_transaction_order(1_500_000_000, 300);
        assert!(
            order1.fee_per_byte > order2.fee_per_byte,
            "{} must be larger than {}",
            order1.fee_per_byte,
            order2.fee_per_byte
        );
        assert_eq!(Ordering::Greater, order1.cmp(&order2));
    }

    #[test]
    fn fee_per_byte_order_sort() {
        let factors: Vec<Vec<usize>> = vec![
            vec![4, 9],   // 19607
            vec![2, 9],   // 9803
            vec![2, 6],   // 11494
            vec![10, 10], // 46728
            vec![2, 8],   // 10309
        ];
        let mut orders: Vec<TransactionOrder> = Vec::new();
        for factor in factors {
            let fee = 1_000_000 * (factor[0] as u64);
            orders.push(create_transaction_order(fee, 10 * factor[1]));
        }

        let prev_orders = orders.clone();
        orders.sort_unstable();
        let sorted_orders = orders;
        assert_eq!(prev_orders[1], sorted_orders[0]);
        assert_eq!(prev_orders[4], sorted_orders[1]);
        assert_eq!(prev_orders[2], sorted_orders[2]);
        assert_eq!(prev_orders[0], sorted_orders[3]);
        assert_eq!(prev_orders[3], sorted_orders[4]);
    }

    fn create_transaction_order(fee: u64, transaction_count: usize) -> TransactionOrder {
        let keypair = Random.generate().unwrap();
        let tx = Transaction {
            seq: 0,
            fee,
            network_id: "tc".into(),
            action: Action::MintAsset {
                network_id: "tc".into(),
                shard_id: 0,
                metadata: String::from_utf8(vec![b'a'; transaction_count]).unwrap(),
                approver: None,
                administrator: None,
                allowed_script_hashes: vec![],
                output: Box::new(AssetMintOutput {
                    lock_script_hash: H160::zero(),
                    parameters: vec![],
                    amount: None,
                }),
                approvals: vec![],
            },
        };
        let timelock = TxTimelock {
            block: None,
            timestamp: None,
        };
        let signed = SignedTransaction::new_with_sign(tx, keypair.private());
        let item = MemPoolItem::new(signed, TxOrigin::Local, 0, 0, timelock);
        TransactionOrder::for_transaction(&item, 0)
    }
}

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
use std::collections::{BTreeSet, HashMap};

use ctypes::{Address, H256, U256};
use heapsize::HeapSizeOf;
use linked_hash_map::LinkedHashMap;
use multimap::MultiMap;
use table::Table;

use super::super::transaction::{Action, SignedTransaction, TransactionError};
use super::super::types::BlockNumber;
use super::local_transactions::{LocalTransactionsList, Status as LocalTransactionStatus};
use super::TransactionImportResult;

/// Transaction with the same (sender, nonce) can be replaced only if
/// `new_fee > old_fee + old_fee >> SHIFT`
const FEE_BUMP_SHIFT: usize = 3; // 2 = 25%, 3 = 12.5%, 4 = 6.25%

/// Point in time when transaction was inserted.
pub type QueuingInstant = BlockNumber;
const DEFAULT_QUEUING_PERIOD: BlockNumber = 128;

/// Transaction origin
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransactionOrigin {
    /// Transaction coming from local RPC
    Local,
    /// External transaction received from network
    External,
    /// Transactions from retracted blocks
    RetractedBlock,
}

impl PartialOrd for TransactionOrigin {
    fn partial_cmp(&self, other: &TransactionOrigin) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TransactionOrigin {
    fn cmp(&self, other: &TransactionOrigin) -> Ordering {
        if *other == *self {
            return Ordering::Equal
        }

        match (*self, *other) {
            (TransactionOrigin::RetractedBlock, _) => Ordering::Less,
            (_, TransactionOrigin::RetractedBlock) => Ordering::Greater,
            (TransactionOrigin::Local, _) => Ordering::Less,
            _ => Ordering::Greater,
        }
    }
}

impl TransactionOrigin {
    fn is_local(&self) -> bool {
        *self == TransactionOrigin::Local
    }
}

#[derive(Clone, Debug)]
/// Light structure used to identify transaction and its order
struct TransactionOrder {
    /// Primary ordering factory. Difference between transaction nonce and expected nonce in state
    /// (e.g. Tx(nonce:5), State(nonce:0) -> height: 5)
    /// High nonce_height = Low priority (processed later)
    nonce_height: U256,
    /// Fee of the transaction.
    fee: U256,
    /// Heap usage of this transaction.
    mem_usage: usize,
    /// Hash to identify associated transaction
    hash: H256,
    /// Incremental id assigned when transaction is inserted to the queue.
    insertion_id: u64,
    /// Origin of the transaction
    origin: TransactionOrigin,
}

impl TransactionOrder {
    fn for_transaction(tx: &QueuedTransaction, base_nonce: U256) -> Self {
        Self {
            nonce_height: tx.nonce() - base_nonce,
            fee: tx.transaction.fee,
            mem_usage: tx.transaction.heap_size_of_children(),
            hash: tx.hash(),
            insertion_id: tx.insertion_id,
            origin: tx.origin,
        }
    }

    fn update_height(mut self, nonce: U256, base_nonce: U256) -> Self {
        self.nonce_height = nonce - base_nonce;
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

        // Check nonce_height
        if self.nonce_height != b.nonce_height {
            return self.nonce_height.cmp(&b.nonce_height)
        }

        // Then compare fee
        if self.fee != b.fee {
            return b.fee.cmp(&self.fee)
        }

        // Lastly compare insertion_id
        self.insertion_id.cmp(&b.insertion_id)
    }
}

/// Queued transaction
#[derive(Debug)]
struct QueuedTransaction {
    /// Transaction.
    transaction: SignedTransaction,
    /// Transaction origin.
    origin: TransactionOrigin,
    /// Insertion time
    insertion_time: QueuingInstant,
    /// ID assigned upon insertion, should be unique.
    insertion_id: u64,
}

impl QueuedTransaction {
    fn new(
        transaction: SignedTransaction,
        origin: TransactionOrigin,
        insertion_time: QueuingInstant,
        insertion_id: u64,
    ) -> Self {
        QueuedTransaction {
            transaction,
            origin,
            insertion_time,
            insertion_id,
        }
    }

    fn hash(&self) -> H256 {
        self.transaction.hash()
    }

    fn nonce(&self) -> U256 {
        self.transaction.nonce
    }

    fn sender(&self) -> Address {
        self.transaction.sender()
    }

    fn cost(&self) -> U256 {
        let value = match (*self.transaction).action {
            Action::Payment {
                value,
                ..
            } => value,
            _ => U256::from(0),
        };
        value + self.transaction.fee
    }
}

/// Holds transactions accessible by (address, nonce) and by priority
struct TransactionSet {
    by_priority: BTreeSet<TransactionOrder>,
    by_address: Table<Address, U256, TransactionOrder>,
    by_fee: MultiMap<U256, H256>,
    limit: usize,
    memory_limit: usize,
}

impl TransactionSet {
    /// Inserts `TransactionOrder` to this set. Transaction does not need to be unique -
    /// the same transaction may be validly inserted twice. Any previous transaction that
    /// it replaces (i.e. with the same `sender` and `nonce`) should be returned.
    fn insert(&mut self, sender: Address, nonce: U256, order: TransactionOrder) -> Option<TransactionOrder> {
        if !self.by_priority.insert(order.clone()) {
            return Some(order.clone())
        }
        let order_hash = order.hash.clone();
        let order_fee = order.fee.clone();
        let by_address_replaced = self.by_address.insert(sender, nonce, order);
        if let Some(ref old_order) = by_address_replaced {
            assert!(
                self.by_priority.remove(old_order),
                "hash is in `by_address`; all transactions in `by_address` must be in `by_priority`; qed"
            );
            assert!(
                self.by_fee.remove(&old_order.fee, &old_order.hash),
                "hash is in `by_address`; all transactions' fee in `by_address` must be in `by_fee`; qed"
            );
        }
        self.by_fee.insert(order_fee, order_hash);
        assert_eq!(self.by_priority.len(), self.by_address.len());
        assert_eq!(self.by_fee.values().map(|v| v.len()).fold(0, |a, b| a + b), self.by_address.len());
        by_address_replaced
    }

    /// Remove low priority transactions if there is more than specified by given `limit`.
    ///
    /// It drops transactions from this set but also removes associated `VerifiedTransaction`.
    /// Returns addresses and lowest nonces of transactions removed because of limit.
    fn enforce_limit(
        &mut self,
        by_hash: &mut HashMap<H256, QueuedTransaction>,
        local: &mut LocalTransactionsList,
    ) -> Option<HashMap<Address, U256>> {
        let mut count = 0;
        let mut mem_usage = 0;
        let to_drop: Vec<(Address, U256)> = {
            self.by_priority
                .iter()
                .filter(|order| {
                    // update transaction count and mem usage
                    count += 1;
                    mem_usage += order.mem_usage;

                    let is_own_or_retracted =
                        order.origin.is_local() || order.origin == TransactionOrigin::RetractedBlock;
                    // Own and retracted transactions are allowed to go above all limits.
                    !is_own_or_retracted && (mem_usage > self.memory_limit || count > self.limit)
                })
                .map(|order| {
                    by_hash.get(&order.hash).expect(
                        "All transactions in `self.by_priority` and `self.by_address` are kept in sync with `by_hash`.",
                    )
                })
                .map(|tx| (tx.sender(), tx.nonce()))
                .collect()
        };

        Some(to_drop.into_iter().fold(HashMap::new(), |mut removed, (sender, nonce)| {
            let order = self.drop(&sender, &nonce)
                .expect("Transaction has just been found in `by_priority`; so it is in `by_address` also.");
            trace!(target: "txqueue", "Dropped out of limit transaction: {:?}", order.hash);

            let order = by_hash
                .remove(&order.hash)
                .expect("hash is in `by_priorty`; all hashes in `by_priority` must be in `by_hash`; qed");

            if order.origin.is_local() {
                local.mark_dropped(order.transaction);
            }

            let min = removed.get(&sender).map_or(nonce, |val| cmp::min(*val, nonce));
            removed.insert(sender, min);
            removed
        }))
    }

    /// Drop transaction from this set (remove from `by_priority` and `by_address`)
    fn drop(&mut self, sender: &Address, nonce: &U256) -> Option<TransactionOrder> {
        if let Some(tx_order) = self.by_address.remove(sender, nonce) {
            assert!(
                self.by_fee.remove(&tx_order.fee, &tx_order.hash),
                "hash is in `by_address`; all transactions' fee in `by_address` must be in `by_fee`; qed"
            );
            assert!(
                self.by_priority.remove(&tx_order),
                "hash is in `by_address`; all transactions in `by_address` must be in `by_priority`; qed"
            );
            assert_eq!(self.by_priority.len(), self.by_address.len());
            assert_eq!(self.by_fee.values().map(|v| v.len()).fold(0, |a, b| a + b), self.by_address.len());
            return Some(tx_order)
        }
        assert_eq!(self.by_priority.len(), self.by_address.len());
        assert_eq!(self.by_fee.values().map(|v| v.len()).fold(0, |a, b| a + b), self.by_address.len());
        None
    }

    /// Drop all transactions.
    fn clear(&mut self) {
        self.by_priority.clear();
        self.by_address.clear();
    }

    /// Sets new limit for number of transactions in this `TransactionSet`.
    /// Note the limit is not applied (no transactions are removed) by calling this method.
    fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }

    /// Get the minimum fee that we can accept into this queue that wouldn't cause the transaction to
    /// immediately be dropped. 0 if the queue isn't at capacity; 1 plus the lowest if it is.
    fn fee_entry_limit(&self) -> U256 {
        match self.by_fee.keys().next() {
            Some(k) if self.by_priority.len() >= self.limit => *k + 1.into(),
            _ => U256::default(),
        }
    }
}

pub struct TransactionQueue {
    /// Fee threshold for transactions that can be imported to this queue (defaults to 0)
    minimal_fee: U256,
    /// Maximal time transaction may occupy the queue.
    /// When we reach `max_time_in_queue / 2^3` we re-validate
    /// account balance.
    max_time_in_queue: QueuingInstant,
    /// Priority queue for transactions that can go to block
    current: TransactionSet,
    /// Priority queue for transactions that has been received but are not yet valid to go to block
    future: TransactionSet,
    /// All transactions managed by queue indexed by hash
    by_hash: HashMap<H256, QueuedTransaction>,
    /// Last nonce of transaction in current (to quickly check next expected transaction)
    last_nonces: HashMap<Address, U256>,
    /// List of local transactions and their statuses.
    local_transactions: LocalTransactionsList,
    /// Next id that should be assigned to a transaction imported to the queue.
    next_transaction_id: u64,
}

impl Default for TransactionQueue {
    fn default() -> Self {
        TransactionQueue::new()
    }
}

impl TransactionQueue {
    /// Creates new instance of this Queue
    pub fn new() -> Self {
        Self::with_limits(8192, usize::max_value())
    }

    /// Create new instance of this Queue with specified limits
    pub fn with_limits(limit: usize, memory_limit: usize) -> Self {
        let current = TransactionSet {
            by_priority: BTreeSet::new(),
            by_address: Table::new(),
            by_fee: MultiMap::default(),
            limit,
            memory_limit,
        };

        let future = TransactionSet {
            by_priority: BTreeSet::new(),
            by_address: Table::new(),
            by_fee: MultiMap::default(),
            limit,
            memory_limit,
        };

        TransactionQueue {
            minimal_fee: U256::zero(),
            max_time_in_queue: DEFAULT_QUEUING_PERIOD,
            current,
            future,
            by_hash: HashMap::new(),
            last_nonces: HashMap::new(),
            local_transactions: LocalTransactionsList::default(),
            next_transaction_id: 0,
        }
    }

    /// Set the new limit for `current` and `future` queue.
    pub fn set_limit(&mut self, limit: usize) {
        self.current.set_limit(limit);
        self.future.set_limit(limit);
        // And ensure the limits
        self.current.enforce_limit(&mut self.by_hash, &mut self.local_transactions);
        self.future.enforce_limit(&mut self.by_hash, &mut self.local_transactions);
    }

    /// Returns current limit of transactions in the queue.
    pub fn limit(&self) -> usize {
        self.current.limit
    }

    /// Get the minimal fee.
    pub fn minimal_fee(&self) -> &U256 {
        &self.minimal_fee
    }

    /// Sets new fee threshold for incoming transactions.
    /// Any transaction already imported to the queue is not affected.
    pub fn set_minimal_fee(&mut self, min_fee: U256) {
        self.minimal_fee = min_fee;
    }

    /// Get one more than the lowest fee in the queue iff the pool is
    /// full, otherwise 0.
    pub fn effective_minimum_fee(&self) -> U256 {
        self.current.fee_entry_limit()
    }

    /// Returns current status for this queue
    pub fn status(&self) -> TransactionQueueStatus {
        TransactionQueueStatus {
            pending: self.current.by_priority.len(),
            future: self.future.by_priority.len(),
        }
    }

    /// Add signed transaction to queue to be verified and imported.
    ///
    /// NOTE details_provider methods should be cheap to compute
    /// otherwise it might open up an attack vector.
    pub fn add(
        &mut self,
        tx: SignedTransaction,
        origin: TransactionOrigin,
        time: QueuingInstant,
        details_provider: &TransactionDetailsProvider,
    ) -> Result<TransactionImportResult, TransactionError> {
        if origin == TransactionOrigin::Local {
            let hash = tx.hash();
            let cloned_tx = tx.clone();

            let result = self.add_internal(tx, origin, time, details_provider);
            match result {
                Ok(TransactionImportResult::Current) => {
                    self.local_transactions.mark_pending(hash);
                }
                Ok(TransactionImportResult::Future) => {
                    self.local_transactions.mark_future(hash);
                }
                Err(ref err) => {
                    // Sometimes transactions are re-imported, so
                    // don't overwrite transactions if they are already on the list
                    if !self.local_transactions.contains(&hash) {
                        self.local_transactions.mark_rejected(cloned_tx, err.clone());
                    }
                }
            }
            result
        } else {
            self.add_internal(tx, origin, time, details_provider)
        }
    }

    /// Removes all transactions from particular sender up to (excluding) given client (state) nonce.
    /// Client (State) Nonce = next valid nonce for this sender.
    pub fn cull(&mut self, sender: Address, client_nonce: U256) {
        // Check if there is anything in current...
        let should_check_in_current = self.current.by_address.row(&sender)
            // If nonce == client_nonce nothing is changed
            .and_then(|by_nonce| by_nonce.keys().find(|nonce| *nonce < &client_nonce))
            .map(|_| ());
        // ... or future
        let should_check_in_future = self.future.by_address.row(&sender)
            // if nonce == client_nonce we need to promote to current
            .and_then(|by_nonce| by_nonce.keys().find(|nonce| *nonce <= &client_nonce))
            .map(|_| ());

        if should_check_in_current.or(should_check_in_future).is_none() {
            return
        }

        self.cull_internal(sender, client_nonce);
    }

    /// Removes all elements (in any state) from the queue
    pub fn clear(&mut self) {
        self.current.clear();
        self.future.clear();
        self.by_hash.clear();
        self.last_nonces.clear();
    }

    /// Finds transaction in the queue by hash (if any)
    pub fn find(&self, hash: &H256) -> Option<SignedTransaction> {
        self.by_hash.get(hash).map(|tx| tx.transaction.clone())
    }

    /// Returns highest transaction nonce for given address.
    pub fn last_nonce(&self, address: &Address) -> Option<U256> {
        self.last_nonces.get(address).cloned()
    }

    /// Returns top transactions from the queue ordered by priority.
    pub fn top_transactions(&self) -> Vec<SignedTransaction> {
        self.current
            .by_priority
            .iter()
            .map(|t| {
                self.by_hash
                    .get(&t.hash)
                    .expect("All transactions in `current` and `future` are always included in `by_hash`")
            })
            .map(|t| t.transaction.clone())
            .collect()
    }

    /// Return all future transactions.
    pub fn future_transactions(&self) -> Vec<SignedTransaction> {
        self.future.by_priority
            .iter()
            .map(|t| self.by_hash.get(&t.hash).expect("All transactions in `current` and `future` are always included in `by_hash`"))
            .map(|t| t.transaction.clone())
            .collect()
    }

    /// Returns local transactions (some of them might not be part of the queue anymore).
    pub fn local_transactions(&self) -> &LinkedHashMap<H256, LocalTransactionStatus> {
        self.local_transactions.all_transactions()
    }

    /// Adds signed transaction to the queue.
    fn add_internal(
        &mut self,
        tx: SignedTransaction,
        origin: TransactionOrigin,
        time: QueuingInstant,
        details_provider: &TransactionDetailsProvider,
    ) -> Result<TransactionImportResult, TransactionError> {
        if origin != TransactionOrigin::Local && tx.fee < self.minimal_fee {
            trace!(target: "txqueue",
                   "Dropping transaction below minimal fee: {:?} (gp: {} < {})",
                   tx.hash(),
                   tx.fee,
                   self.minimal_fee
            );

            return Err(TransactionError::InsufficientFee {
                minimal: self.minimal_fee,
                got: tx.fee,
            })
        }

        let full_queues_lowest = self.effective_minimum_fee();
        if tx.fee < full_queues_lowest && origin != TransactionOrigin::Local {
            trace!(target: "txqueue",
                   "Dropping transaction below lowest fee in a full queue: {:?} (gp: {} < {})",
                   tx.hash(),
                   tx.fee,
                   full_queues_lowest
            );

            return Err(TransactionError::InsufficientFee {
                minimal: full_queues_lowest,
                got: tx.fee,
            })
        }

        let client_account = details_provider.fetch_account(&tx.sender());
        if client_account.balance < tx.fee {
            trace!(target: "txqueue",
                   "Dropping transaction without sufficient balance: {:?} ({} < {})",
                   tx.hash(),
                   client_account.balance,
                   tx.fee
            );

            return Err(TransactionError::InsufficientBalance {
                cost: tx.fee,
                balance: client_account.balance,
            })
        }
        tx.check_low_s()?;
        // No invalid transactions beyond this point.
        let id = self.next_transaction_id;
        self.next_transaction_id += 1;
        let vtx = QueuedTransaction::new(tx, origin, time, id);
        let r = self.import_transaction(vtx, client_account.nonce);
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
        r
    }

    /// Adds VerifiedTransaction to this queue.
    ///
    /// Determines if it should be placed in current or future. When transaction is
    /// imported to `current` also checks if there are any `future` transactions that should be promoted because of
    /// this.
    ///
    /// It ignores transactions that has already been imported (same `hash`) and replaces the transaction
    /// iff `(address, nonce)` is the same but `fee` is higher.
    ///
    /// Returns `true` when transaction was imported successfully
    fn import_transaction(
        &mut self,
        tx: QueuedTransaction,
        state_nonce: U256,
    ) -> Result<TransactionImportResult, TransactionError> {
        if self.by_hash.get(&tx.hash()).is_some() {
            // Transaction is already imported.
            trace!(target: "txqueue", "Dropping already imported transaction: {:?}", tx.hash());
            return Err(TransactionError::AlreadyImported)
        }

        let address = tx.sender();
        let nonce = tx.nonce();
        let hash = tx.hash();

        // The transaction might be old, let's check that.
        // This has to be the first test, otherwise calculating
        // nonce height would result in overflow.
        if nonce < state_nonce {
            // Droping transaction
            trace!(target: "txqueue", "Dropping old transaction: {:?} (nonce: {} < {})", tx.hash(), nonce, state_nonce);
            return Err(TransactionError::Old)
        }

        // Update nonces of transactions in future (remove old transactions)
        self.update_future(&address, state_nonce);
        // State nonce could be updated. Maybe there are some more items waiting in future?
        self.move_matching_future_to_current(address, state_nonce, state_nonce);
        // Check the next expected nonce (might be updated by move above)
        let next_nonce = self.last_nonces.get(&address).cloned().map_or(state_nonce, |n| n + U256::one());

        if tx.origin.is_local() {
            self.mark_transactions_local(&address);
        }

        // Future transaction
        if nonce > next_nonce {
            // We have a gap - put to future.
            // Insert transaction (or replace old one with lower fee)
            check_too_cheap(Self::replace_transaction(
                tx,
                state_nonce,
                &mut self.future,
                &mut self.by_hash,
                &mut self.local_transactions,
            ))?;
            // Enforce limit in Future
            let removed = self.future.enforce_limit(&mut self.by_hash, &mut self.local_transactions);
            // Return an error if this transaction was not imported because of limit.
            check_if_removed(&address, &nonce, removed)?;

            debug!(target: "txqueue", "Importing transaction to future: {:?}", hash);
            debug!(target: "txqueue", "status: {:?}", self.status());
            return Ok(TransactionImportResult::Future)
        }

        // We might have filled a gap - move some more transactions from future
        self.move_matching_future_to_current(address, nonce, state_nonce);
        self.move_matching_future_to_current(address, nonce + U256::one(), state_nonce);

        // Replace transaction if any
        check_too_cheap(Self::replace_transaction(
            tx,
            state_nonce,
            &mut self.current,
            &mut self.by_hash,
            &mut self.local_transactions,
        ))?;
        // Keep track of highest nonce stored in current
        let new_max = self.last_nonces.get(&address).map_or(nonce, |n| cmp::max(nonce, *n));
        self.last_nonces.insert(address, new_max);

        // Also enforce the limit
        let removed = self.current.enforce_limit(&mut self.by_hash, &mut self.local_transactions);
        // If some transaction were removed because of limit we need to update last_nonces also.
        self.update_last_nonces(&removed);
        // Trigger error if the transaction we are importing was removed.
        check_if_removed(&address, &nonce, removed)?;

        debug!(target: "txqueue", "Imported transaction to current: {:?}", hash);
        debug!(target: "txqueue", "status: {:?}", self.status());
        Ok(TransactionImportResult::Current)
    }

    /// Always updates future and moves transactions from current to future.
    fn cull_internal(&mut self, sender: Address, client_nonce: U256) {
        // We will either move transaction to future or remove it completely
        // so there will be no transactions from this sender in current
        self.last_nonces.remove(&sender);
        // First update height of transactions in future to avoid collisions
        self.update_future(&sender, client_nonce);
        // This should move all current transactions to future and remove old transactions
        self.move_all_to_future(&sender, client_nonce);
        // And now lets check if there is some batch of transactions in future
        // that should be placed in current. It should also update last_nonces.
        self.move_matching_future_to_current(sender, client_nonce, client_nonce);
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
    }

    fn update_last_nonces(&mut self, removed_min_nonces: &Option<HashMap<Address, U256>>) {
        if let Some(ref min_nonces) = *removed_min_nonces {
            for (sender, nonce) in min_nonces.iter() {
                if *nonce == U256::zero() {
                    self.last_nonces.remove(sender);
                } else {
                    self.last_nonces.insert(*sender, *nonce - U256::one());
                }
            }
        }
    }

    /// Update height of all transactions in future transactions set.
    fn update_future(&mut self, sender: &Address, current_nonce: U256) {
        // We need to drain all transactions for current sender from future and reinsert them with updated height
        let all_nonces_from_sender = match self.future.by_address.row(sender) {
            Some(row_map) => row_map.keys().cloned().collect::<Vec<U256>>(),
            None => vec![],
        };
        for k in all_nonces_from_sender {
            let order =
                self.future.drop(sender, &k).expect("iterating over a collection that has been retrieved above; qed");
            if k >= current_nonce {
                self.future.insert(*sender, k, order.update_height(k, current_nonce));
            } else {
                trace!(target: "txqueue", "Removing old transaction: {:?} (nonce: {} < {})", order.hash, k, current_nonce);
                // Remove the transaction completely
                self.by_hash.remove(&order.hash).expect("All transactions in `future` are also in `by_hash`");
            }
        }
    }

    /// Checks if there are any transactions in `future` that should actually be promoted to `current`
    /// (because nonce matches).
    fn move_matching_future_to_current(&mut self, address: Address, mut current_nonce: U256, first_nonce: U256) {
        let mut update_last_nonce_to = None;
        {
            let by_nonce = self.future.by_address.row_mut(&address);
            if by_nonce.is_none() {
                return
            }
            let by_nonce = by_nonce.expect("None is tested in early-exit condition above; qed");
            while let Some(order) = by_nonce.remove(&current_nonce) {
                // remove also from priority and fee
                self.future.by_priority.remove(&order);
                self.future.by_fee.remove(&order.fee, &order.hash);
                // Put to current
                let order = order.update_height(current_nonce, first_nonce);
                if order.origin.is_local() {
                    self.local_transactions.mark_pending(order.hash);
                }
                if let Some(old) = self.current.insert(address, current_nonce, order.clone()) {
                    Self::replace_orders(
                        address,
                        current_nonce,
                        old,
                        order,
                        &mut self.current,
                        &mut self.by_hash,
                        &mut self.local_transactions,
                    );
                }
                update_last_nonce_to = Some(current_nonce);
                current_nonce = current_nonce + U256::one();
            }
        }
        self.future.by_address.clear_if_empty(&address);
        if let Some(x) = update_last_nonce_to {
            // Update last inserted nonce
            self.last_nonces.insert(address, x);
        }
    }

    /// Drop all transactions from given sender from `current`.
    /// Either moves them to `future` or removes them from queue completely.
    fn move_all_to_future(&mut self, sender: &Address, current_nonce: U256) {
        let all_nonces_from_sender = match self.current.by_address.row(sender) {
            Some(row_map) => row_map.keys().cloned().collect::<Vec<U256>>(),
            None => vec![],
        };

        for k in all_nonces_from_sender {
            // Goes to future or is removed
            let order =
                self.current.drop(sender, &k).expect("iterating over a collection that has been retrieved above; qed");
            if k >= current_nonce {
                let order = order.update_height(k, current_nonce);
                if order.origin.is_local() {
                    self.local_transactions.mark_future(order.hash);
                }
                if let Some(old) = self.future.insert(*sender, k, order.clone()) {
                    Self::replace_orders(
                        *sender,
                        k,
                        old,
                        order,
                        &mut self.future,
                        &mut self.by_hash,
                        &mut self.local_transactions,
                    );
                }
            } else {
                trace!(target: "txqueue", "Removing old transaction: {:?} (nonce: {} < {})", order.hash, k, current_nonce);
                let tx = self.by_hash.remove(&order.hash).expect("All transactions in `future` are also in `by_hash`");
                if tx.origin.is_local() {
                    self.local_transactions.mark_mined(tx.transaction);
                }
            }
        }
        self.future.enforce_limit(&mut self.by_hash, &mut self.local_transactions);
    }

    /// Marks all transactions from particular sender as local transactions
    fn mark_transactions_local(&mut self, sender: &Address) {
        fn mark_local<F: FnMut(H256)>(sender: &Address, set: &mut TransactionSet, mut mark: F) {
            // Mark all transactions from this sender as local
            let nonces_from_sender = set.by_address
                .row(sender)
                .map(|row_map| {
                    row_map
                        .iter()
                        .filter_map(|(nonce, order)| {
                            if order.origin.is_local() {
                                None
                            } else {
                                Some(*nonce)
                            }
                        })
                        .collect::<Vec<U256>>()
                })
                .unwrap_or_else(Vec::new);

            for k in nonces_from_sender {
                let mut order = set.drop(sender, &k).expect("transaction known to be in self.current/self.future; qed");
                order.origin = TransactionOrigin::Local;
                mark(order.hash);
                set.insert(*sender, k, order);
            }
        }

        let local = &mut self.local_transactions;
        mark_local(sender, &mut self.current, |hash| local.mark_pending(hash));
        mark_local(sender, &mut self.future, |hash| local.mark_future(hash));
    }

    /// Replaces transaction in given set (could be `future` or `current`).
    ///
    /// If there is already transaction with same `(sender, nonce)` it will be replaced iff `fee` is higher.
    /// One of the transactions is dropped from set and also removed from queue entirely (from `by_hash`).
    ///
    /// Returns `true` if transaction actually got to the queue (`false` if there was already a transaction with higher
    /// fee)
    fn replace_transaction(
        tx: QueuedTransaction,
        base_nonce: U256,
        set: &mut TransactionSet,
        by_hash: &mut HashMap<H256, QueuedTransaction>,
        local: &mut LocalTransactionsList,
    ) -> bool {
        let order = TransactionOrder::for_transaction(&tx, base_nonce);
        let hash = tx.hash();
        let address = tx.sender();
        let nonce = tx.nonce();

        let old_hash = by_hash.insert(hash, tx);
        assert!(old_hash.is_none(), "Each hash has to be inserted exactly once.");

        trace!(target: "txqueue", "Inserting: {:?}", order);

        if let Some(old) = set.insert(address, nonce, order.clone()) {
            Self::replace_orders(address, nonce, old, order, set, by_hash, local)
        } else {
            true
        }
    }

    fn replace_orders(
        address: Address,
        nonce: U256,
        old: TransactionOrder,
        order: TransactionOrder,
        set: &mut TransactionSet,
        by_hash: &mut HashMap<H256, QueuedTransaction>,
        local: &mut LocalTransactionsList,
    ) -> bool {
        // There was already transaction in queue. Let's check which one should stay
        let old_hash = old.hash;
        let new_hash = order.hash;

        let old_fee = old.fee;
        let new_fee = order.fee;
        let min_required_fee = old_fee + (old_fee >> FEE_BUMP_SHIFT);

        if min_required_fee > new_fee {
            trace!(target: "txqueue", "Didn't insert transaction because fee was too low: {:?} ({:?} stays in the queue)", order.hash, old.hash);
            // Put back old transaction since it has greater priority (higher fee)
            set.insert(address, nonce, old);
            // and remove new one
            let order = by_hash
                .remove(&order.hash)
                .expect("The hash has been just inserted and no other line is altering `by_hash`.");
            if order.origin.is_local() {
                local.mark_replaced(order.transaction, old_fee, old_hash);
            }
            false
        } else {
            trace!(target: "txqueue", "Replaced transaction: {:?} with transaction with higher fee: {:?}", old.hash, order.hash);
            // Make sure we remove old transaction entirely
            let old =
                by_hash.remove(&old.hash).expect("The hash is coming from `future` so it has to be in `by_hash`.");
            if old.origin.is_local() {
                local.mark_replaced(old.transaction, new_fee, new_hash);
            }
            true
        }
    }
}

#[derive(Debug)]
/// Current status of the queue
pub struct TransactionQueueStatus {
    /// Number of pending transactions (ready to go to block)
    pub pending: usize,
    /// Number of future transactions (waiting for transactions with lower nonces first)
    pub future: usize,
}

/// `TransactionQueue` transaction details provider.
pub trait TransactionDetailsProvider {
    /// Fetch transaction-related account details.
    fn fetch_account(&self, address: &Address) -> AccountDetails;
}

/// Details of account
pub struct AccountDetails {
    /// Most recent account nonce
    pub nonce: U256,
    /// Current account balance
    pub balance: U256,
}

fn check_too_cheap(is_in: bool) -> Result<(), TransactionError> {
    if is_in {
        Ok(())
    } else {
        Err(TransactionError::TooCheapToReplace)
    }
}

fn check_if_removed(
    sender: &Address,
    nonce: &U256,
    dropped: Option<HashMap<Address, U256>>,
) -> Result<(), TransactionError> {
    match dropped {
        Some(ref dropped) => match dropped.get(sender) {
            Some(min) if nonce >= min => Err(TransactionError::LimitReached),
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

#[cfg(test)]
pub mod test {
    use super::TransactionOrigin;
    use std::cmp::Ordering;

    #[test]
    fn test_ordering() {
        assert_eq!(TransactionOrigin::Local.cmp(&TransactionOrigin::External), Ordering::Less);
        assert_eq!(TransactionOrigin::RetractedBlock.cmp(&TransactionOrigin::Local), Ordering::Less);
        assert_eq!(TransactionOrigin::RetractedBlock.cmp(&TransactionOrigin::External), Ordering::Less);

        assert_eq!(TransactionOrigin::External.cmp(&TransactionOrigin::Local), Ordering::Greater);
        assert_eq!(TransactionOrigin::Local.cmp(&TransactionOrigin::RetractedBlock), Ordering::Greater);
        assert_eq!(TransactionOrigin::External.cmp(&TransactionOrigin::RetractedBlock), Ordering::Greater);
    }
}

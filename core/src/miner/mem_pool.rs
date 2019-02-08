// Copyright 2018-2019 Kodebox, Inc.
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
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use ckey::{public_to_address, Public};
use ctypes::errors::{HistoryError, RuntimeError, SyntaxError};
use ctypes::BlockNumber;
use kvdb::{DBTransaction, KeyValueDB};
use primitives::H256;
use rlp;
use table::Table;
use time::get_time;

use super::backup;
use super::mem_pool_types::{
    AccountDetails, CurrentQueue, FutureQueue, MemPoolInput, MemPoolItem, MemPoolStatus, PoolingInstant, QueueTag,
    TransactionOrder, TransactionOrderWithTag, TxOrigin, TxTimelock,
};
use super::TransactionImportResult;
use crate::transaction::SignedTransaction;
use crate::Error as CoreError;

const DEFAULT_POOLING_PERIOD: BlockNumber = 128;

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    History(HistoryError),
    Runtime(RuntimeError),
    Syntax(SyntaxError),
}

impl Error {
    pub fn into_core_error(self) -> CoreError {
        match self {
            Error::History(err) => CoreError::History(err),
            Error::Runtime(err) => CoreError::Runtime(err),
            Error::Syntax(err) => CoreError::Syntax(err),
        }
    }
}

impl From<HistoryError> for Error {
    fn from(err: HistoryError) -> Error {
        Error::History(err)
    }
}

impl From<RuntimeError> for Error {
    fn from(err: RuntimeError) -> Error {
        Error::Runtime(err)
    }
}

impl From<SyntaxError> for Error {
    fn from(err: SyntaxError) -> Error {
        Error::Syntax(err)
    }
}

pub struct MemPool {
    /// Fee threshold for transactions that can be imported to this pool (defaults to 0)
    minimal_fee: u64,
    /// A value which is used to check whether a new transaciton can replace a transaction in the memory pool with the same signer and seq.
    /// If the fee of the new transaction is `new_fee` and the fee of the transaction in the memory pool is `old_fee`,
    /// then `new_fee > old_fee + old_fee >> mem_pool_fee_bump_shift` should be satisfied to replace.
    /// Local transactions ignore this option.
    fee_bump_shift: usize,
    /// Maximal time transaction may occupy the pool.
    /// When we reach `max_time_in_pool / 2^3` we re-validate
    /// account balance.
    max_time_in_pool: PoolingInstant,
    /// Priority queue and fee counter for transactions that can go to block
    current: CurrentQueue,
    /// Priority queue for transactions that has been received but are not yet valid to go to block
    future: FutureQueue,
    /// All transactions managed by pool indexed by public and seq
    by_signer_public: Table<Public, u64, TransactionOrderWithTag>,
    /// The count(number) limit of each queue
    queue_count_limit: usize,
    /// The memory limit of each queue
    queue_memory_limit: usize,
    /// All transactions managed by pool indexed by hash
    by_hash: HashMap<H256, MemPoolItem>,
    /// Current seq of each public key (fee payer)
    first_seqs: HashMap<Public, u64>,
    /// Next seq of transaction in current (to quickly check next expected transaction)
    next_seqs: HashMap<Public, u64>,
    /// Check if there's any local transaction from specific account
    is_local_account: HashSet<Public>,
    /// The time when the pool is finally used
    last_time: PoolingInstant,
    /// The timestamp when the pool is finally used
    last_timestamp: u64,
    /// Next id that should be assigned to a transaction imported to the pool
    next_transaction_id: u64,
    /// Arc of KeyValueDB in which the backup information is stored.
    db: Arc<KeyValueDB>,
}

impl MemPool {
    /// Create new instance of this Queue with specified limits
    pub fn with_limits(limit: usize, memory_limit: usize, fee_bump_shift: usize, db: Arc<KeyValueDB>) -> Self {
        let mut result = MemPool {
            minimal_fee: 0,
            fee_bump_shift,
            max_time_in_pool: DEFAULT_POOLING_PERIOD,
            current: CurrentQueue::new(),
            future: FutureQueue::new(),
            by_signer_public: Table::new(),
            queue_count_limit: limit,
            queue_memory_limit: memory_limit,
            by_hash: HashMap::new(),
            first_seqs: HashMap::new(),
            next_seqs: HashMap::new(),
            is_local_account: HashSet::new(),
            last_time: 0,
            last_timestamp: 0,
            next_transaction_id: 0,
            db,
        };
        result.recover_from_db();

        result
    }

    /// Set the new limit for `current` and `future` queue.
    pub fn set_limit(&mut self, limit: usize) {
        self.queue_count_limit = limit;
    }

    /// Enforce the limit to the current/future queue
    fn enforce_limit(&mut self, batch: &mut DBTransaction) {
        // Get transaction orders to drop from each queue (current/future)
        fn get_orders_to_drop(
            set: &BTreeSet<TransactionOrder>,
            limit: usize,
            memory_limit: usize,
        ) -> Vec<TransactionOrder> {
            let mut count = 0;
            let mut mem_usage = 0;
            set.iter()
                .filter(|order| {
                    count += 1;
                    mem_usage += order.mem_usage;
                    !order.origin.is_local_or_retracted() && (mem_usage > memory_limit || count > limit)
                })
                .cloned()
                .collect()
        }

        let to_drop_current =
            if self.current.mem_usage > self.queue_memory_limit || self.current.count > self.queue_count_limit {
                get_orders_to_drop(&self.current.queue, self.queue_count_limit, self.queue_memory_limit)
            } else {
                vec![]
            };

        let to_drop_future =
            if self.future.mem_usage > self.queue_memory_limit || self.future.count > self.queue_count_limit {
                get_orders_to_drop(&self.future.queue, self.queue_count_limit, self.queue_memory_limit)
            } else {
                vec![]
            };

        for (order, is_current) in
            to_drop_current.iter().map(|order| (order, true)).chain(to_drop_future.iter().map(|order| (order, false)))
        {
            let hash = order.hash;
            let item = self.by_hash.remove(&hash).expect("`by_hash` and `current/future` should be synced");
            backup::remove_item(batch, &hash);
            let signer_public = item.signer_public();
            let seq = item.seq();
            self.by_signer_public
                .remove(&signer_public, &seq)
                .expect("`by_hash` and `by_signer_public` should be synced");
            if self.by_signer_public.clear_if_empty(&signer_public) {
                self.is_local_account.remove(&signer_public);
            }
            if is_current {
                self.current.remove(order);
            } else {
                self.future.remove(order);
            }
        }
    }

    /// Returns current limit of transactions in the pool.
    pub fn limit(&self) -> usize {
        self.queue_count_limit
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
        if self.current.len() >= self.queue_count_limit {
            self.current.minimum_fee()
        } else {
            0
        }
    }

    /// Returns current status for this pool
    pub fn status(&self) -> MemPoolStatus {
        MemPoolStatus {
            pending: self.current.len(),
            future: self.future.len(),
        }
    }

    /// Add signed transaction to pool to be verified and imported.
    ///
    /// NOTE details_provider methods should be cheap to compute
    /// otherwise it might open up an attack vector.
    pub fn add<F>(
        &mut self,
        inputs: Vec<MemPoolInput>,
        current_time: PoolingInstant,
        current_timestamp: u64,
        fetch_account: &F,
    ) -> Vec<Result<TransactionImportResult, Error>>
    where
        F: Fn(&Public) -> AccountDetails, {
        ctrace!(MEM_POOL, "add() called, time: {}, timestamp: {}", current_time, current_timestamp);
        let mut insert_results = Vec::new();
        let mut to_insert: HashMap<Public, Vec<u64>> = HashMap::new();
        let mut new_local_accounts = HashSet::new();
        let mut batch = backup::backup_batch_with_capacity(inputs.len());

        for input in inputs {
            let tx = input.transaction;
            let signer_public = tx.signer_public();
            let seq = tx.seq;
            let hash = tx.hash();
            let timelock = input.timelock;

            let origin = if input.origin.is_local() && !self.is_local_account.contains(&signer_public) {
                self.is_local_account.insert(signer_public);
                new_local_accounts.insert(signer_public);
                TxOrigin::Local
            } else if input.origin.is_external() && self.is_local_account.contains(&signer_public) {
                TxOrigin::Local
            } else {
                input.origin
            };

            let client_account = fetch_account(&signer_public);
            if let Err(e) = self.verify_transaction(&tx, origin, &client_account) {
                insert_results.push(Err(e));
                continue
            }

            let id = self.next_transaction_id;
            self.next_transaction_id += 1;
            let item = MemPoolItem::new(tx, origin, current_time, id, timelock);
            let order = TransactionOrder::for_transaction(&item, client_account.seq);
            let order_with_tag = TransactionOrderWithTag::new(order, QueueTag::New);

            backup::backup_item(&mut batch, hash, &item);
            self.by_hash.insert(hash, item);

            if let Some(old_order_with_tag) = self.by_signer_public.insert(signer_public, seq, order_with_tag) {
                let old_order = old_order_with_tag.order;
                let tag = old_order_with_tag.tag;

                self.by_hash.remove(&old_order.hash);
                backup::remove_item(&mut batch, &old_order.hash);

                match tag {
                    QueueTag::Current => {
                        self.current.remove(&old_order);
                    }
                    QueueTag::Future => {
                        self.future.remove(&old_order);
                    }
                    QueueTag::New => unreachable!(),
                }
            }

            to_insert.entry(signer_public).or_default().push(seq);
            insert_results.push(Ok((signer_public, seq)));
        }

        let keys = self.by_signer_public.keys().map(Clone::clone).collect::<Vec<_>>();

        for public in keys {
            let current_seq = fetch_account(&public).seq;
            let mut first_seq = *self.first_seqs.get(&public).unwrap_or(&0);
            let next_seq = self.next_seqs.get(&public).cloned().unwrap_or(current_seq);

            let new_next_seq = if current_seq < first_seq
                || current_time < self.last_time
                || current_timestamp < self.last_timestamp
                || next_seq < current_seq
            {
                self.check_transactions(public, current_seq, current_time, current_timestamp)
            } else {
                to_insert
                    .get(&public)
                    .and_then(|v| self.check_new_transactions(public, v, next_seq, current_time, current_timestamp))
                    .unwrap_or_else(|| self.check_transactions(public, next_seq, current_time, current_timestamp))
            };

            let is_this_account_local = new_local_accounts.contains(&public);
            // Need to update transactions because of height/origin change
            if current_seq != first_seq || is_this_account_local {
                self.update_orders(public, current_seq, new_next_seq, is_this_account_local, &mut batch);
                self.first_seqs.insert(public, current_seq);
                backup::backup_seqs(&mut batch, &public, current_seq, true);
                first_seq = current_seq;
            }
            // We don't need to update the height, just move transactions
            else if new_next_seq < next_seq {
                self.move_queue(public, new_next_seq, next_seq, QueueTag::Future);
            } else if new_next_seq > next_seq {
                self.move_queue(public, next_seq, new_next_seq, QueueTag::Current);
            }


            if new_next_seq <= first_seq {
                self.next_seqs.remove(&public);
                backup::remove_seqs(&mut batch, &public, false);
            } else {
                self.next_seqs.insert(public, new_next_seq);
                backup::backup_seqs(&mut batch, &public, new_next_seq, false);
            }

            if let Some(seq_list) = to_insert.get(&public) {
                self.add_new_orders_to_queue(public, seq_list, new_next_seq);
            }

            if self.by_signer_public.clear_if_empty(&public) {
                self.is_local_account.remove(&public);
            }
        }

        self.enforce_limit(&mut batch);

        self.last_time = current_time;
        self.last_timestamp = current_timestamp;

        assert_eq!(self.current.len() + self.future.len(), self.by_hash.len());
        assert_eq!(self.current.fee_counter.values().sum::<usize>(), self.current.len());
        assert_eq!(self.by_signer_public.len(), self.by_hash.len());

        backup::backup_extra(&mut batch, b"last_time", self.last_time);
        backup::backup_extra(&mut batch, b"last_timestamp", self.last_timestamp);
        backup::backup_extra(&mut batch, b"next_transaction_id", self.next_transaction_id);

        self.db.write(batch).expect("Low level database error. Some issue with disk?");
        insert_results
            .into_iter()
            .map(|v| match v {
                Ok((signer_public, seq)) => match self.by_signer_public.get(&signer_public, &seq) {
                    Some(order_with_tag) => match order_with_tag.tag {
                        QueueTag::Current => Ok(TransactionImportResult::Current),
                        QueueTag::Future => Ok(TransactionImportResult::Future),
                        QueueTag::New => unreachable!(),
                    },
                    None => Err(HistoryError::LimitReached.into()),
                },
                Err(e) => Err(e),
            })
            .collect()
    }

    /// Checks the current seq for all transactions' senders in the pool and removes the old transactions.
    /// Expired transactions are removed by this function only.
    pub fn remove_old<F>(&mut self, fetch_account: &F, current_time: PoolingInstant, current_timestamp: u64)
    where
        F: Fn(&Public) -> AccountDetails, {
        ctrace!(MEM_POOL, "remove_old() called, time: {}, timestamp: {}", current_time, current_timestamp);
        let signers =
            self.by_signer_public.keys().map(|sender| (*sender, fetch_account(sender))).collect::<HashMap<_, _>>();
        let max_time = self.max_time_in_pool;
        let balance_check = max_time >> 3;

        // Clear transactions occupying the pool too long, or expired
        let invalid = self
            .by_hash
            .iter()
            .filter(|&(_, ref item)| !item.origin.is_local())
            .map(|(hash, item)| (hash, item, current_time.saturating_sub(item.insertion_time)))
            .filter_map(|(hash, item, time_diff)| {
                // FIXME: In PoW, current_timestamp can be roll-backed.
                // In that case, transactions which are removed in here can be recovered.
                if let Some(expiration) = item.expiration() {
                    if expiration < current_timestamp {
                        return Some(*hash)
                    }
                }

                if time_diff > max_time {
                    return Some(*hash)
                }

                if time_diff > balance_check {
                    return match signers.get(&item.signer_public()) {
                        Some(details) if item.cost() > details.balance => Some(*hash),
                        _ => None,
                    }
                }

                None
            })
            .collect::<Vec<_>>();
        let fetch_seq =
            |a: &Public| signers.get(a).expect("We fetch details for all signers from both current and future").seq;
        self.remove(&invalid, &fetch_seq, current_time, current_timestamp);
    }

    // Recover MemPool state from db stored data
    fn recover_from_db(&mut self) {
        let backup::RecoveredData {
            by_hash,
            first_seqs,
            next_seqs,
            last_time,
            last_timestamp,
            next_transaction_id,
        } = backup::recover_to_data(self.db.as_ref());

        let mut to_insert: HashMap<_, Vec<_>> = HashMap::new();
        let mut keys = Vec::with_capacity(first_seqs.len());

        for (hash, item) in by_hash.iter() {
            let signer_public = item.signer_public();
            let seq = item.seq();

            let client_account_seq = *first_seqs.get(&signer_public).expect("Low Level Database Error");

            let order = TransactionOrder::for_transaction(&item, client_account_seq);
            let order_with_tag = TransactionOrderWithTag::new(order, QueueTag::New);

            to_insert.entry(signer_public).or_default().push(seq);
            self.by_hash.insert(*hash, item.clone());

            keys.push(signer_public);
            self.by_signer_public.insert(signer_public, seq, order_with_tag);
            if item.origin == TxOrigin::Local {
                self.is_local_account.insert(signer_public);
            }
        }

        let keys = self.by_signer_public.keys().map(Clone::clone).collect::<Vec<_>>();

        for public in keys {
            if let Some(seq_list) = to_insert.get(&public) {
                let next_seq = *next_seqs.get(&public).expect("Low Level Database Error");
                self.add_new_orders_to_queue(public, seq_list, next_seq);
            }
        }

        self.next_seqs = next_seqs;
        self.first_seqs = first_seqs;
        self.last_time = last_time;
        self.last_timestamp = last_timestamp;
        self.next_transaction_id = next_transaction_id;
    }

    /// Removes invalid transaction identified by hash from pool.
    /// Assumption is that this transaction seq is not related to client seq,
    /// so transactions left in pool are processed according to client seq.
    ///
    /// If gap is introduced marks subsequent transactions as future
    pub fn remove<F>(
        &mut self,
        transaction_hashes: &[H256],
        fetch_seq: &F,
        current_time: PoolingInstant,
        current_timestamp: u64,
    ) where
        F: Fn(&Public) -> u64, {
        ctrace!(MEM_POOL, "remove() called, time: {}, timestamp: {}", current_time, current_timestamp);
        let mut removed: HashMap<_, _> = HashMap::new();
        let mut batch = backup::backup_batch_with_capacity(transaction_hashes.len());

        for hash in transaction_hashes {
            if let Some(item) = self.by_hash.get(hash).map(Clone::clone) {
                let signer_public = item.signer_public();
                let seq = item.seq();
                let current_seq = fetch_seq(&signer_public);

                let order_with_tag = *self
                    .by_signer_public
                    .get(&signer_public, &seq)
                    .expect("`by_hash` and `by_signer_public` must be synced");
                let order = order_with_tag.order;
                match order_with_tag.tag {
                    QueueTag::Current => self.current.remove(&order),
                    QueueTag::Future => self.future.remove(&order),
                    QueueTag::New => unreachable!(),
                }

                self.by_hash.remove(hash);
                backup::remove_item(&mut batch, hash);
                self.by_signer_public.remove(&signer_public, &seq);
                if current_seq <= seq {
                    let old = removed.get(&signer_public).map(Clone::clone);
                    match old {
                        Some(old_seq) if old_seq <= seq => {}
                        _ => {
                            removed.insert(signer_public, seq);
                        }
                    }
                }
            }
        }

        let keys = self.by_signer_public.keys().map(Clone::clone).collect::<Vec<_>>();

        for public in keys {
            let current_seq = fetch_seq(&public);
            let mut first_seq = *self.first_seqs.get(&public).unwrap_or(&0);
            let next_seq = self.next_seqs.get(&public).cloned().unwrap_or(current_seq);

            let new_next_seq = if current_seq < first_seq
                || current_time < self.last_time
                || current_timestamp < self.last_timestamp
                || next_seq < current_seq
            {
                self.check_transactions(public, current_seq, current_time, current_timestamp)
            } else if let Some(seq) = removed.get(&public) {
                *seq
            } else {
                self.check_transactions(public, next_seq, current_time, current_timestamp)
            };

            // Need to update the height
            if current_seq != first_seq {
                self.update_orders(public, current_seq, new_next_seq, false, &mut batch);
                self.first_seqs.insert(public, current_seq);
                backup::backup_seqs(&mut batch, &public, current_seq, true);
                first_seq = current_seq;
            }
            // We don't need to update the height, just move transactions
            else if new_next_seq < next_seq {
                self.move_queue(public, new_next_seq, next_seq, QueueTag::Future);
            } else if new_next_seq > next_seq {
                self.move_queue(public, next_seq, new_next_seq, QueueTag::Current);
            }


            if new_next_seq <= first_seq {
                self.next_seqs.remove(&public);
                backup::remove_seqs(&mut batch, &public, false);
            } else {
                self.next_seqs.insert(public, new_next_seq);
                backup::backup_seqs(&mut batch, &public, new_next_seq, false);
            }

            if self.by_signer_public.clear_if_empty(&public) {
                self.is_local_account.remove(&public);
            }
        }

        self.last_time = current_time;
        self.last_timestamp = current_timestamp;

        assert_eq!(self.current.len() + self.future.len(), self.by_hash.len());
        assert_eq!(self.current.fee_counter.values().sum::<usize>(), self.current.len());
        assert_eq!(self.by_signer_public.len(), self.by_hash.len());

        backup::backup_extra(&mut batch, b"last_time", self.last_time);
        backup::backup_extra(&mut batch, b"last_timestamp", self.last_timestamp);
        backup::backup_extra(&mut batch, b"next_transaction_id", self.next_transaction_id);

        self.db.write(batch).expect("Low level database error. Some issue with disk?");
    }

    /// Checks the timelock of transactions starting from `start_seq`.
    /// Returns the next seq of the last transaction which can be in the current queue
    fn check_transactions(
        &self,
        public: Public,
        mut start_seq: u64,
        current_time: PoolingInstant,
        current_timestamp: u64,
    ) -> u64 {
        let row = self
            .by_signer_public
            .row(&public)
            .expect("This function should be called after checking from `self.by_signer_public.keys()`");

        while let Some(order_with_tag) = row.get(&start_seq) {
            let order = order_with_tag.order;
            if Self::should_wait_timelock(&order.timelock, current_time, current_timestamp) {
                break
            }
            start_seq += 1;
        }

        start_seq
    }

    /// Checks the timelock of transactions with the given seqs.
    /// If there are transactions which should wait timelock, returns the smallest seq by Some(seq).
    /// If there's no transaction which should wait timelock, returns None.
    fn check_new_transactions(
        &self,
        public: Public,
        seqs: &[u64],
        next_seq: u64,
        current_time: PoolingInstant,
        current_timestamp: u64,
    ) -> Option<u64> {
        let row = self
            .by_signer_public
            .row(&public)
            .expect("This function should be called after checking from `self.by_signer_public.keys()`");

        let mut result = None;

        for seq in seqs {
            if *seq >= next_seq {
                continue
            }
            let order_with_tag = row.get(&seq).expect("Must exist");
            let order = order_with_tag.order;
            if Self::should_wait_timelock(&order.timelock, current_time, current_timestamp)
                && (result.is_none() || (result.is_some() && result.unwrap() > *seq))
            {
                result = Some(*seq)
            }
        }

        result
    }

    /// Moves the transactions which of seq is in [start_seq, end_seq -1],
    /// to the given queue `to`.
    fn move_queue(&mut self, public: Public, mut start_seq: u64, end_seq: u64, to: QueueTag) {
        let row = self
            .by_signer_public
            .row_mut(&public)
            .expect("This function should be called after checking from `self.by_signer_public.keys()`");

        while start_seq < end_seq {
            if let Some(order_with_tag) = row.get_mut(&start_seq) {
                let tag = order_with_tag.tag;
                match tag {
                    QueueTag::Current if to == QueueTag::Future => {
                        let order = order_with_tag.order;
                        order_with_tag.tag = QueueTag::Future;
                        self.current.remove(&order);
                        self.future.insert(order);
                    }
                    QueueTag::Future if to == QueueTag::Current => {
                        let order = order_with_tag.order;
                        order_with_tag.tag = QueueTag::Current;
                        self.future.remove(&order);
                        self.current.insert(order);
                    }
                    _ => {}
                }
            }
            start_seq += 1;
        }
    }

    /// Add the given transactions to the corresponding queue.
    /// It should be tagged as QueueTag::New in self.by_signer_public.
    fn add_new_orders_to_queue(&mut self, public: Public, seq_list: &[u64], new_next_seq: u64) {
        let row = self
            .by_signer_public
            .row_mut(&public)
            .expect("This function should be called after checking from `self.by_signer_public.keys()`");

        for seq in seq_list {
            let order_with_tag = row.get_mut(seq).expect("Must exist");
            let tag = order_with_tag.tag;
            match tag {
                QueueTag::New => {
                    let order = order_with_tag.order;
                    if *seq < new_next_seq {
                        order_with_tag.tag = QueueTag::Current;
                        self.current.insert(order);
                    } else {
                        order_with_tag.tag = QueueTag::Future;
                        self.future.insert(order);
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    /// Updates the seq height of the orders in the queues and self.by_signer_public.
    /// Also, drops old transactions.
    fn update_orders(
        &mut self,
        public: Public,
        current_seq: u64,
        new_next_seq: u64,
        to_local: bool,
        batch: &mut DBTransaction,
    ) {
        let row = self
            .by_signer_public
            .row_mut(&public)
            .expect("This function should be called after checking from `self.by_signer_public.keys()`");

        let seqs = row.keys().map(Clone::clone).collect::<Vec<_>>();

        for seq in seqs {
            let order_with_tag = *row.get(&seq).expect("Must exist");
            let old_order = order_with_tag.order;

            // Remove old order
            match order_with_tag.tag {
                QueueTag::Current => self.current.remove(&old_order),
                QueueTag::Future => self.future.remove(&old_order),
                QueueTag::New => continue,
            }
            row.remove(&seq);

            if seq < current_seq {
                self.by_hash.remove(&old_order.hash);
                backup::remove_item(batch, &old_order.hash);
            } else {
                let new_order = old_order.update_height(seq, current_seq);
                let new_order = if to_local {
                    new_order.change_origin(TxOrigin::Local)
                } else {
                    new_order
                };
                if seq < new_next_seq {
                    let new_order_with_tag = TransactionOrderWithTag::new(new_order, QueueTag::Current);
                    self.current.insert(new_order);
                    row.insert(seq, new_order_with_tag);
                } else {
                    let new_order_with_tag = TransactionOrderWithTag::new(new_order, QueueTag::Future);
                    self.future.insert(new_order);
                    row.insert(seq, new_order_with_tag);
                }
            }
        }
    }

    /// Verify signed transaction with its content.
    /// This function can return errors: InsufficientFee, InsufficientBalance,
    /// TransactionAlreadyImported, Old, TooCheapToReplace
    fn verify_transaction(
        &self,
        tx: &SignedTransaction,
        origin: TxOrigin,
        client_account: &AccountDetails,
    ) -> Result<(), Error> {
        if origin != TxOrigin::Local && tx.fee < self.minimal_fee {
            ctrace!(
                MEM_POOL,
                "Dropping transaction below minimal fee: {:?} (gp: {} < {})",
                tx.hash(),
                tx.fee,
                self.minimal_fee
            );

            return Err(SyntaxError::InsufficientFee {
                minimal: self.minimal_fee,
                got: tx.fee,
            }
            .into())
        }

        let full_pools_lowest = self.effective_minimum_fee();
        if origin != TxOrigin::Local && tx.fee < full_pools_lowest {
            ctrace!(
                MEM_POOL,
                "Dropping transaction below lowest fee in a full pool: {:?} (gp: {} < {})",
                tx.hash(),
                tx.fee,
                full_pools_lowest
            );

            return Err(SyntaxError::InsufficientFee {
                minimal: full_pools_lowest,
                got: tx.fee,
            }
            .into())
        }

        if client_account.balance < tx.fee {
            ctrace!(
                MEM_POOL,
                "Dropping transaction without sufficient balance: {:?} ({} < {})",
                tx.hash(),
                client_account.balance,
                tx.fee
            );

            return Err(RuntimeError::InsufficientBalance {
                address: public_to_address(&tx.signer_public()),
                cost: tx.fee,
                balance: client_account.balance,
            }
            .into())
        }

        if self.by_hash.get(&tx.hash()).is_some() {
            ctrace!(MEM_POOL, "Dropping already imported transaction: {:?}", tx.hash());
            return Err(HistoryError::TransactionAlreadyImported.into())
        }

        if tx.seq < client_account.seq {
            ctrace!(MEM_POOL, "Dropping old transaction: {:?} (seq: {} < {})", tx.hash(), tx.seq, client_account.seq);
            return Err(HistoryError::Old.into())
        }

        if origin != TxOrigin::Local {
            if let Some(TransactionOrderWithTag {
                order,
                ..
            }) = self.by_signer_public.get(&tx.signer_public(), &tx.seq)
            {
                let old_fee = order.fee;
                let new_fee = tx.fee;
                let min_required_fee = old_fee + (old_fee >> self.fee_bump_shift);

                if new_fee < min_required_fee {
                    ctrace!(
                        MEM_POOL,
                        "Dropping transaction because fee is not enough to replace: {:?} (gp: {} < {}) (old_fee: {})",
                        tx.hash(),
                        new_fee,
                        min_required_fee,
                        old_fee,
                    );
                    return Err(HistoryError::TooCheapToReplace.into())
                }
            }
        }

        Ok(())
    }

    /// Removes all elements (in any state) from the pool
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.current.clear();
        self.future.clear();
        self.by_signer_public.clear();
        self.by_hash.clear();
        self.first_seqs.clear();
        self.next_seqs.clear();
    }

    /// Returns top transactions from the pool ordered by priority.
    // FIXME: current_timestamp should be `u64`, not `Option<u64>`.
    pub fn top_transactions(&self, size_limit: usize, current_timestamp: Option<u64>) -> Vec<SignedTransaction> {
        let mut current_size: usize = 0;
        self.current
            .queue
            .iter()
            .map(|t| {
                self.by_hash
                    .get(&t.hash)
                    .expect("All transactions in `current` and `future` are always included in `by_hash`")
            })
            .filter(|t| {
                if let Some(expiration) = t.expiration() {
                    if let Some(timestamp) = current_timestamp {
                        return expiration >= timestamp
                    }
                }
                true
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

    /// Return all transactions in the memory pool.
    pub fn count_pending_transactions(&self) -> usize {
        self.current.queue.len() + self.future.queue.len()
    }

    /// Return all future transactions.
    pub fn future_transactions(&self) -> Vec<SignedTransaction> {
        self.future
            .queue
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
        self.current.queue.iter().any(|tx| tx.origin.is_local())
    }

    /// Checks the given timelock with the current time/timestamp.
    fn should_wait_timelock(timelock: &TxTimelock, best_block_number: BlockNumber, best_block_timestamp: u64) -> bool {
        if let Some(block_number) = timelock.block {
            if block_number > best_block_number + 1 {
                return true
            }
        }
        if let Some(timestamp) = timelock.timestamp {
            if timestamp > cmp::max(get_time().sec as u64, best_block_timestamp) {
                return true
            }
        }
        false
    }
}


#[cfg(test)]
pub mod test {
    use std::cmp::Ordering;

    use ckey::{Generator, Random};
    use ctypes::transaction::{Action, AssetMintOutput, Transaction};
    use primitives::H160;

    use super::*;
    use rlp::rlp_encode_and_decode_test;

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
                    supply: None,
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
                metadata: "".into(),
                approvals: vec![],
                expiration: None,
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
        let quantity = 100_000;
        let receiver = 1u64.into();
        let keypair = Random.generate().unwrap();
        let tx = Transaction {
            seq: 0,
            fee,
            network_id: "tc".into(),
            action: Action::Pay {
                receiver,
                quantity,
            },
        };
        let timelock = TxTimelock {
            block: None,
            timestamp: None,
        };
        let signed = SignedTransaction::new_with_sign(tx, keypair.private());
        let item = MemPoolItem::new(signed, TxOrigin::Local, 0, 0, timelock);

        assert_eq!(fee + quantity, item.cost());
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

    #[test]
    fn txorigin_encode_and_decode() {
        rlp_encode_and_decode_test!(TxOrigin::External);
    }

    #[test]
    fn txtimelock_encode_and_decode() {
        let timelock = TxTimelock {
            block: None,
            timestamp: None,
        };
        rlp_encode_and_decode_test!(timelock);
    }

    #[test]
    fn signed_transaction_encode_and_decode() {
        let receiver = 0u64.into();
        let keypair = Random.generate().unwrap();
        let tx = Transaction {
            seq: 0,
            fee: 100,
            network_id: "tc".into(),
            action: Action::Pay {
                receiver,
                quantity: 100_000,
            },
        };
        let signed = SignedTransaction::new_with_sign(tx, keypair.private());

        rlp_encode_and_decode_test!(signed);
    }

    #[test]
    fn mempool_item_encode_and_decode() {
        let keypair = Random.generate().unwrap();
        let tx = Transaction {
            seq: 0,
            fee: 10,
            network_id: "tc".into(),
            action: Action::MintAsset {
                network_id: "tc".into(),
                shard_id: 0,
                metadata: String::from_utf8(vec![b'a'; 1]).unwrap(),
                approver: None,
                administrator: None,
                allowed_script_hashes: vec![],
                output: Box::new(AssetMintOutput {
                    lock_script_hash: H160::zero(),
                    parameters: vec![],
                    supply: None,
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

        rlp_encode_and_decode_test!(item);
    }

    #[test]
    fn db_backup_and_recover() {
        let db = Arc::new(kvdb_memorydb::create(crate::db::NUM_COLUMNS.unwrap_or(0)));
        let mut mem_pool = MemPool::with_limits(8192, usize::max_value(), 3, db.clone());
        let fetch_account = |_p: &Public| -> AccountDetails {
            AccountDetails {
                seq: 0,
                balance: u64::max_value(),
            }
        };
        let current_time = 100;
        let current_timestamp = 100;
        let mut inputs: Vec<MemPoolInput> = Vec::new();

        let input1 = create_mempool_input_with_pay(100, 100_000, 1u64);
        let input2 = create_mempool_input_with_pay(100, 200_000, 2u64);

        inputs.push(input1);
        inputs.push(input2);

        mem_pool.add(inputs, current_time, current_timestamp, &fetch_account);

        let mem_pool_recovered = MemPool::with_limits(8192, usize::max_value(), 3, db.clone());

        assert_eq!(mem_pool_recovered.last_time, mem_pool.last_time);
        assert_eq!(mem_pool_recovered.last_timestamp, mem_pool.last_timestamp);
        assert_eq!(mem_pool_recovered.first_seqs, mem_pool.first_seqs);
        assert_eq!(mem_pool_recovered.next_seqs, mem_pool.next_seqs);
        assert_eq!(mem_pool_recovered.is_local_account, mem_pool.is_local_account);
        assert_eq!(mem_pool_recovered.next_transaction_id, mem_pool.next_transaction_id);
        assert_eq!(mem_pool_recovered.by_hash, mem_pool.by_hash);
        assert_eq!(mem_pool_recovered.queue_count_limit, mem_pool.queue_count_limit);
        assert_eq!(mem_pool_recovered.queue_memory_limit, mem_pool.queue_memory_limit);
        assert_eq!(mem_pool_recovered.current, mem_pool.current);
        assert_eq!(mem_pool_recovered.future, mem_pool.future);
    }

    fn create_mempool_input_with_pay(fee: u64, quantity: u64, reciever_u64: u64) -> MemPoolInput {
        let receiver = reciever_u64.into();
        let keypair = Random.generate().unwrap();
        let tx = Transaction {
            seq: 0,
            fee,
            network_id: "tc".into(),
            action: Action::Pay {
                receiver,
                quantity,
            },
        };
        let timelock = TxTimelock {
            block: None,
            timestamp: None,
        };
        let signed = SignedTransaction::new_with_sign(tx, keypair.private());

        MemPoolInput::new(signed, TxOrigin::Local, timelock)
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
                    supply: None,
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

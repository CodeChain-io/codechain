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
use multimap::MultiMap;
use table::Table;

use super::super::transaction::{Action, SignedTransaction};
use super::super::types::BlockNumber;
use super::local_transactions::LocalTransactionsList;

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
    fn for_transaction(tx: &VerifiedTransaction, base_nonce: U256) -> Self {
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

/// Verified transaction
#[derive(Debug)]
struct VerifiedTransaction {
    /// Transaction.
    transaction: SignedTransaction,
    /// Transaction origin.
    origin: TransactionOrigin,
    /// Insertion time
    insertion_time: QueuingInstant,
    /// ID assigned upon insertion, should be unique.
    insertion_id: u64,
}

/// Point in time when transaction was inserted.
pub type QueuingInstant = BlockNumber;

impl VerifiedTransaction {
    fn new(
        transaction: SignedTransaction,
        origin: TransactionOrigin,
        insertion_time: QueuingInstant,
        insertion_id: u64,
    ) -> Self {
        VerifiedTransaction {
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
        by_hash: &mut HashMap<H256, VerifiedTransaction>,
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

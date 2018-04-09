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

use super::super::transaction::{SignedTransaction, TransactionError};
use ctypes::{H256, U256};
use linked_hash_map::LinkedHashMap;

/// Status of local transaction.
/// Can indicate that the transaction is currently part of the queue (`Pending/Future`)
/// or gives a reason why the transaction was removed.
#[derive(Debug, PartialEq, Clone)]
pub enum Status {
    /// The transaction is currently in the transaction queue.
    Pending,
    /// The transaction is in future part of the queue.
    Future,
    /// Transaction is already mined.
    Mined(SignedTransaction),
    /// Transaction is dropped because of limit
    Dropped(SignedTransaction),
    /// Replaced because of higher gas price of another transaction.
    Replaced(SignedTransaction, U256, H256),
    /// Transaction was never accepted to the queue.
    Rejected(SignedTransaction, TransactionError),
    /// Transaction is invalid.
    Invalid(SignedTransaction),
    /// Transaction was canceled.
    Canceled(SignedTransaction),
}

impl Status {
    fn is_current(&self) -> bool {
        *self == Status::Pending || *self == Status::Future
    }
}

/// Keeps track of local transactions that are in the queue or were mined/dropped recently.
#[derive(Debug)]
pub struct LocalTransactionsList {
    max_old: usize,
    transactions: LinkedHashMap<H256, Status>,
}

impl Default for LocalTransactionsList {
    fn default() -> Self {
        Self::new(10)
    }
}

impl LocalTransactionsList {
    /// Create a new list of local transactions.
    pub fn new(max_old: usize) -> Self {
        LocalTransactionsList {
            max_old,
            transactions: Default::default(),
        }
    }

    /// Mark transaction with given hash as pending.
    pub fn mark_pending(&mut self, hash: H256) {
        debug!(target: "own_tx", "Imported to Current (hash {:?})", hash);
        self.clear_old();
        self.transactions.insert(hash, Status::Pending);
    }

    /// Mark transaction with given hash as future.
    pub fn mark_future(&mut self, hash: H256) {
        debug!(target: "own_tx", "Imported to Future (hash {:?})", hash);
        self.transactions.insert(hash, Status::Future);
        self.clear_old();
    }

    /// Mark given transaction as rejected from the queue.
    pub fn mark_rejected(&mut self, tx: SignedTransaction, err: TransactionError) {
        debug!(target: "own_tx", "Transaction rejected (hash {:?}): {:?}", tx.hash(), err);
        self.transactions.insert(tx.hash(), Status::Rejected(tx, err));
        self.clear_old();
    }

    /// Mark the transaction as replaced by transaction with given hash.
    pub fn mark_replaced(&mut self, tx: SignedTransaction, gas_price: U256, hash: H256) {
        debug!(target: "own_tx", "Transaction replaced (hash {:?}) by {:?} (new gas price: {:?})", tx.hash(), hash, gas_price);
        self.transactions.insert(tx.hash(), Status::Replaced(tx, gas_price, hash));
        self.clear_old();
    }

    /// Mark transaction as invalid.
    pub fn mark_invalid(&mut self, tx: SignedTransaction) {
        warn!(target: "own_tx", "Transaction marked invalid (hash {:?})", tx.hash());
        self.transactions.insert(tx.hash(), Status::Invalid(tx));
        self.clear_old();
    }

    /// Mark transaction as canceled.
    pub fn mark_canceled(&mut self, tx: SignedTransaction) {
        warn!(target: "own_tx", "Transaction canceled (hash {:?})", tx.hash());
        self.transactions.insert(tx.hash(), Status::Canceled(tx));
        self.clear_old();
    }

    /// Mark transaction as dropped because of limit.
    pub fn mark_dropped(&mut self, tx: SignedTransaction) {
        warn!(target: "own_tx", "Transaction dropped (hash {:?})", tx.hash());
        self.transactions.insert(tx.hash(), Status::Dropped(tx));
        self.clear_old();
    }

    /// Mark transaction as mined.
    pub fn mark_mined(&mut self, tx: SignedTransaction) {
        info!(target: "own_tx", "Transaction mined (hash {:?})", tx.hash());
        self.transactions.insert(tx.hash(), Status::Mined(tx));
        self.clear_old();
    }

    /// Returns true if the transaction is already in local transactions.
    pub fn contains(&self, hash: &H256) -> bool {
        self.transactions.contains_key(hash)
    }

    /// Return a map of all currently stored transactions.
    pub fn all_transactions(&self) -> &LinkedHashMap<H256, Status> {
        &self.transactions
    }

    fn clear_old(&mut self) {
        let number_of_old = self.transactions.values().filter(|status| !status.is_current()).count();

        if self.max_old >= number_of_old {
            return
        }

        let to_remove = self.transactions
            .iter()
            .filter(|&(_, status)| !status.is_current())
            .map(|(hash, _)| *hash)
            .take(number_of_old - self.max_old)
            .collect::<Vec<_>>();

        for hash in to_remove {
            self.transactions.remove(&hash);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ckeys::{Generator, Random};
    use ctypes::U256;

    #[test]
    fn should_add_transaction_as_pending() {
        // given
        let mut list = LocalTransactionsList::default();

        // when
        list.mark_pending(10.into());
        list.mark_future(20.into());

        // then
        assert!(list.contains(&10.into()), "Should contain the transaction.");
        assert!(list.contains(&20.into()), "Should contain the transaction.");
        let statuses = list.all_transactions().values().cloned().collect::<Vec<Status>>();
        assert_eq!(statuses, vec![Status::Pending, Status::Future]);
    }

    #[test]
    fn should_clear_old_transactions() {
        // given
        let mut list = LocalTransactionsList::new(1);
        let tx1 = new_tx(10.into());
        let tx1_hash = tx1.hash();
        let tx2 = new_tx(50.into());
        let tx2_hash = tx2.hash();

        list.mark_pending(10.into());
        list.mark_invalid(tx1);
        list.mark_dropped(tx2);
        assert!(list.contains(&tx2_hash));
        assert!(!list.contains(&tx1_hash));
        assert!(list.contains(&10.into()));

        // when
        list.mark_future(15.into());

        // then
        assert!(list.contains(&10.into()));
        assert!(list.contains(&15.into()));
    }

    fn new_tx(nonce: U256) -> SignedTransaction {
        let keypair = Random.generate().unwrap();
        transaction::Transaction {
            nonce,
            fee: U256::from(1245),
            action: transaction::Action::Noop,
            data: Default::default(),
            network_id: 0u64,
        }.sign(keypair.private())
    }
}

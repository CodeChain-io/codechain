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

use ctypes::transaction::ParcelError;
use linked_hash_map::LinkedHashMap;
use primitives::H256;

use crate::transaction::SignedTransaction;

/// Status of local transaction.
/// Can indicate that the transaction is currently part of the queue (`Pending/Future`)
/// or gives a reason why the transaction was removed.
#[derive(Debug, PartialEq, Clone)]
pub enum Status {
    /// The transaction is currently in the mem pool.
    Pending,
    /// The transaction is in future part of the mem pool.
    Future,
    /// Transaction is already mined.
    Mined(Box<SignedTransaction>),
    /// Transaction is dropped because of limit
    Dropped(Box<SignedTransaction>),
    /// Replaced because of higher gas price of another transaction.
    Replaced(Box<SignedTransaction>, u64, Box<H256>),
    /// Transaction was never accepted to the mem pool.
    Rejected(Box<SignedTransaction>, Box<ParcelError>),
    /// Transaction is invalid.
    Invalid(Box<SignedTransaction>),
    /// Transaction was canceled.
    Canceled(Box<SignedTransaction>),
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
        cdebug!(OWN_PARCEL, "Imported to Current (hash {:?})", hash);
        self.clear_old();
        self.transactions.insert(hash, Status::Pending);
    }

    /// Mark transaction with given hash as future.
    pub fn mark_future(&mut self, hash: H256) {
        cdebug!(OWN_PARCEL, "Imported to Future (hash {:?})", hash);
        self.transactions.insert(hash, Status::Future);
        self.clear_old();
    }

    /// Mark given transaction as rejected from the queue.
    pub fn mark_rejected(&mut self, tx: SignedTransaction, err: ParcelError) {
        cdebug!(OWN_PARCEL, "Tx rejected (hash {:?}): {:?}", tx.hash(), err);
        self.transactions.insert(tx.hash(), Status::Rejected(tx.into(), err.into()));
        self.clear_old();
    }

    /// Mark the transaction as replaced by transaction with given hash.
    pub fn mark_replaced(&mut self, tx: SignedTransaction, gas_price: u64, hash: H256) {
        cdebug!(OWN_PARCEL, "Tx replaced (hash {:?}) by {:?} (new gas price: {:?})", tx.hash(), hash, gas_price);
        self.transactions.insert(tx.hash(), Status::Replaced(tx.into(), gas_price, hash.into()));
        self.clear_old();
    }

    /// Mark transaction as invalid.
    pub fn mark_invalid(&mut self, signed: SignedTransaction) {
        cwarn!(OWN_PARCEL, "Tx marked invalid (hash {:?})", signed.hash());
        self.transactions.insert(signed.hash(), Status::Invalid(signed.into()));
        self.clear_old();
    }

    /// Mark transaction as canceled.
    pub fn mark_canceled(&mut self, signed: SignedTransaction) {
        cwarn!(OWN_PARCEL, "Tx canceled (hash {:?})", signed.hash());
        self.transactions.insert(signed.hash(), Status::Canceled(signed.into()));
        self.clear_old();
    }

    /// Mark transaction as dropped because of limit.
    pub fn mark_dropped(&mut self, signed: SignedTransaction) {
        cwarn!(OWN_PARCEL, "Tx dropped (hash {:?})", signed.hash());
        self.transactions.insert(signed.hash(), Status::Dropped(signed.into()));
        self.clear_old();
    }

    /// Mark transaction as mined.
    pub fn mark_mined(&mut self, signed: SignedTransaction) {
        cinfo!(OWN_PARCEL, "Tx mined (hash {:?})", signed.hash());
        self.transactions.insert(signed.hash(), Status::Mined(signed.into()));
        self.clear_old();
    }

    /// Returns true if the transaction is already in local transactions.
    pub fn contains(&self, hash: &H256) -> bool {
        self.transactions.contains_key(hash)
    }

    /// Return a map of all currently stored tranasctions.
    pub fn all_transactions(&self) -> &LinkedHashMap<H256, Status> {
        &self.transactions
    }

    fn clear_old(&mut self) {
        let number_of_old = self.transactions.values().filter(|status| !status.is_current()).count();

        if self.max_old >= number_of_old {
            return
        }

        let to_remove = self
            .transactions
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
    use ckey::{Generator, Random};
    use ctypes::transaction::{Action, Transaction};

    #[test]
    fn add_tranasction_as_pending() {
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
    fn clear_old_tranasctions() {
        // given
        let mut list = LocalTransactionsList::new(1);
        let tx1 = new_transaction(10);
        let tx1_hash = tx1.hash();
        let tx2 = new_transaction(50);
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

    fn new_transaction(seq: u64) -> SignedTransaction {
        let keypair = Random.generate().unwrap();
        let tx = Transaction {
            seq,
            fee: 1245,
            action: Action::Pay {
                receiver: keypair.address(),
                amount: 0,
            },
            network_id: "tc".into(),
        };
        SignedTransaction::new_with_sign(tx, keypair.private())
    }
}

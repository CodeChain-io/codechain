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

use std::cmp::Ordering;

use ctypes::{Address, H256, U256};
use heapsize::HeapSizeOf;

use super::super::transaction::{SignedTransaction, Action};
use super::super::types::BlockNumber;

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
            return Ordering::Equal;
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
            return self.origin.cmp(&b.origin);
        }

        // Check nonce_height
        if self.nonce_height != b.nonce_height {
            return self.nonce_height.cmp(&b.nonce_height);
        }

        // Then compare fee
        if self.fee != b.fee {
            return b.fee.cmp(&self.fee);
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
            Action::Payment { value, .. } => value,
            _ => U256::from(0),
        };
        value + self.transaction.fee
    }
}

#[cfg(test)]
pub mod test {
    use std::cmp::Ordering;
    use super::TransactionOrigin;

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


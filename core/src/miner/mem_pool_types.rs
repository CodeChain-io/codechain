// Copyright 2019 Kodebox, Inc.
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
use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of_val;

use ckey::Public;
use ctypes::transaction::Action;
use ctypes::BlockNumber;
use heapsize::HeapSizeOf;
use primitives::H256;
use rlp;

use crate::transaction::SignedTransaction;

/// Point in time when transaction was inserted.
pub type PoolingInstant = BlockNumber;

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
    pub fn is_local(self) -> bool {
        self == TxOrigin::Local
    }

    pub fn is_local_or_retracted(self) -> bool {
        self == TxOrigin::Local || self == TxOrigin::RetractedBlock
    }

    pub fn is_external(self) -> bool {
        self == TxOrigin::External
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TxTimelock {
    pub block: Option<BlockNumber>,
    pub timestamp: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
/// Light structure used to identify transaction and its order
pub struct TransactionOrder {
    /// Primary ordering factory. Difference between transaction seq and expected seq in state
    /// (e.g. Transaction(seq:5), State(seq:0) -> height: 5)
    /// High seq_height = Low priority (processed later)
    pub seq_height: u64,
    /// Fee of the transaction.
    pub fee: u64,
    /// Fee per bytes(rlp serialized) of the transaction
    pub fee_per_byte: u64,
    /// Heap usage of this transaction.
    pub mem_usage: usize,
    /// Hash to identify associated transaction
    pub hash: H256,
    /// Incremental id assigned when transaction is inserted to the pool.
    pub insertion_id: u64,
    /// Origin of the transaction
    pub origin: TxOrigin,
    /// Timelock
    pub timelock: TxTimelock,
}

impl TransactionOrder {
    pub fn for_transaction(item: &MemPoolItem, seq_seq: u64) -> Self {
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

    pub fn update_height(mut self, seq: u64, base_seq: u64) -> Self {
        self.seq_height = seq - base_seq;
        self
    }

    pub fn change_origin(mut self, origin: TxOrigin) -> Self {
        self.origin = origin;
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
#[derive(Clone, Debug)]
pub struct MemPoolItem {
    /// Transaction.
    pub tx: SignedTransaction,
    /// Transaction origin.
    pub origin: TxOrigin,
    /// Insertion time
    pub insertion_time: PoolingInstant,
    /// ID assigned upon insertion, should be unique.
    pub insertion_id: u64,
    /// A timelock.
    pub timelock: TxTimelock,
}

impl MemPoolItem {
    pub fn new(
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

    pub fn hash(&self) -> H256 {
        self.tx.hash()
    }

    pub fn seq(&self) -> u64 {
        self.tx.seq
    }

    pub fn signer_public(&self) -> Public {
        self.tx.signer_public()
    }

    pub fn cost(&self) -> u64 {
        match &self.tx.action {
            Action::Pay {
                quantity,
                ..
            } => self.tx.fee + *quantity,
            Action::WrapCCC {
                quantity,
                ..
            } => self.tx.fee + *quantity,
            _ => self.tx.fee,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QueueTag {
    Current,
    Future,
    New,
}

#[derive(Clone, Copy, Debug)]
pub struct TransactionOrderWithTag {
    pub order: TransactionOrder,
    pub tag: QueueTag,
}

impl TransactionOrderWithTag {
    pub fn new(order: TransactionOrder, tag: QueueTag) -> Self {
        Self {
            order,
            tag,
        }
    }
}

pub struct CurrentQueue {
    /// Priority queue for transactions
    pub queue: BTreeSet<TransactionOrder>,
    /// Counter on fees of transactions in the current queue
    pub fee_counter: BTreeMap<u64, usize>,
    /// Memory usage of the external transactions in the queue
    pub mem_usage: usize,
    /// Count of the external transactions in the queue
    pub count: usize,
}

impl CurrentQueue {
    pub fn new() -> Self {
        Self {
            queue: BTreeSet::new(),
            fee_counter: BTreeMap::new(),
            mem_usage: 0,
            count: 0,
        }
    }

    pub fn clear(&mut self) {
        self.queue.clear();
        self.fee_counter.clear();
        self.mem_usage = 0;
        self.count = 0;
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn insert(&mut self, order: TransactionOrder) {
        self.queue.insert(order);
        if !order.origin.is_local_or_retracted() {
            self.mem_usage += order.mem_usage;
            self.count += 1;
        }
        *self.fee_counter.entry(order.fee).or_default() += 1;
    }

    pub fn remove(&mut self, order: &TransactionOrder) {
        assert!(self.queue.remove(order));
        if !order.origin.is_local_or_retracted() {
            self.mem_usage -= order.mem_usage;
            self.count -= 1;
        }
        {
            let counter = self.fee_counter.get_mut(&order.fee).unwrap();
            *counter -= 1;
            if *counter != 0 {
                return
            }
        }
        self.fee_counter.remove(&order.fee);
    }

    pub fn minimum_fee(&self) -> u64 {
        self.fee_counter.keys().next().map_or(0, |k| k + 1)
    }
}

pub struct FutureQueue {
    /// Priority queue for transactions
    pub queue: BTreeSet<TransactionOrder>,
    /// Memory usage of the external transactions in the queue
    pub mem_usage: usize,
    /// Count of the external transactions in the queue
    pub count: usize,
}

impl FutureQueue {
    pub fn new() -> Self {
        Self {
            queue: BTreeSet::new(),
            mem_usage: 0,
            count: 0,
        }
    }

    pub fn clear(&mut self) {
        self.queue.clear();
        self.mem_usage = 0;
        self.count = 0;
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn insert(&mut self, order: TransactionOrder) {
        self.queue.insert(order);
        if !order.origin.is_local_or_retracted() {
            self.mem_usage += order.mem_usage;
            self.count += 1;
        }
    }

    pub fn remove(&mut self, order: &TransactionOrder) {
        assert!(self.queue.remove(order));
        if !order.origin.is_local_or_retracted() {
            self.mem_usage -= order.mem_usage;
            self.count -= 1;
        }
    }
}

#[derive(Clone, Debug)]
pub struct MemPoolInput {
    pub transaction: SignedTransaction,
    pub origin: TxOrigin,
    pub timelock: TxTimelock,
}

impl MemPoolInput {
    pub fn new(transaction: SignedTransaction, origin: TxOrigin, timelock: TxTimelock) -> Self {
        Self {
            transaction,
            origin,
            timelock,
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

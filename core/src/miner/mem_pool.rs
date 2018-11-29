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

use ckey::{public_to_address, Public};
use ctypes::parcel::{Action, Error as ParcelError};
use ctypes::BlockNumber;
use heapsize::HeapSizeOf;
use linked_hash_map::LinkedHashMap;
use multimap::MultiMap;
use primitives::H256;
use rlp;
use table::Table;
use time::get_time;

use super::local_parcels::{LocalParcelsList, Status as LocalParcelStatus};
use super::ParcelImportResult;
use crate::parcel::SignedParcel;

/// Parcel with the same (sender, seq) can be replaced only if
/// `new_fee > old_fee + old_fee >> SHIFT`
const FEE_BUMP_SHIFT: usize = 3; // 2 = 25%, 3 = 12.5%, 4 = 6.25%

/// Point in time when parcel was inserted.
pub type PoolingInstant = BlockNumber;
const DEFAULT_POOLING_PERIOD: BlockNumber = 128;

/// Parcel origin
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParcelOrigin {
    /// Parcel coming from local RPC
    Local,
    /// External parcel received from network
    External,
    /// Parcel from retracted blocks
    RetractedBlock,
}

impl PartialOrd for ParcelOrigin {
    fn partial_cmp(&self, other: &ParcelOrigin) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ParcelOrigin {
    fn cmp(&self, other: &ParcelOrigin) -> Ordering {
        if *other == *self {
            return Ordering::Equal
        }

        match (*self, *other) {
            (ParcelOrigin::RetractedBlock, _) => Ordering::Less,
            (_, ParcelOrigin::RetractedBlock) => Ordering::Greater,
            (ParcelOrigin::Local, _) => Ordering::Less,
            _ => Ordering::Greater,
        }
    }
}

impl ParcelOrigin {
    fn is_local(self) -> bool {
        self == ParcelOrigin::Local
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ParcelTimelock {
    pub block: Option<BlockNumber>,
    pub timestamp: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
/// Light structure used to identify parcel and its order
struct ParcelOrder {
    /// Primary ordering factory. Difference between parcel seq and expected seq in state
    /// (e.g. Parcel(seq:5), State(seq:0) -> height: 5)
    /// High seq_height = Low priority (processed later)
    seq_height: u64,
    /// Fee of the parcel.
    fee: u64,
    /// Fee per bytes(rlp serialized) of the parcel
    fee_per_byte: u64,
    /// Heap usage of this parcel.
    mem_usage: usize,
    /// Hash to identify associated parcel
    hash: H256,
    /// Incremental id assigned when parcel is inserted to the pool.
    insertion_id: u64,
    /// Origin of the parcel
    origin: ParcelOrigin,
    /// Timelock
    timelock: ParcelTimelock,
}

impl ParcelOrder {
    fn for_parcel(item: &MemPoolItem, seq_seq: u64) -> Self {
        let rlp_bytes_len = rlp::encode(&item.parcel).to_vec().len();
        let fee = item.parcel.fee;
        ctrace!(MEM_POOL, "New parcel with size {}", item.parcel.heap_size_of_children());
        Self {
            seq_height: item.seq() - seq_seq,
            fee,
            mem_usage: item.parcel.heap_size_of_children(),
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

impl Eq for ParcelOrder {}
impl PartialEq for ParcelOrder {
    fn eq(&self, other: &ParcelOrder) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl PartialOrd for ParcelOrder {
    fn partial_cmp(&self, other: &ParcelOrder) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ParcelOrder {
    fn cmp(&self, b: &ParcelOrder) -> Ordering {
        // Local parcels should always have priority
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

/// Parcel item in the mem pool.
#[derive(Debug)]
struct MemPoolItem {
    /// Parcel.
    parcel: SignedParcel,
    /// Parcel origin.
    origin: ParcelOrigin,
    /// Insertion time
    insertion_time: PoolingInstant,
    /// ID assigned upon insertion, should be unique.
    insertion_id: u64,
    /// A timelock.
    timelock: ParcelTimelock,
}

impl MemPoolItem {
    fn new(
        parcel: SignedParcel,
        origin: ParcelOrigin,
        insertion_time: PoolingInstant,
        insertion_id: u64,
        timelock: ParcelTimelock,
    ) -> Self {
        MemPoolItem {
            parcel,
            origin,
            insertion_time,
            insertion_id,
            timelock,
        }
    }

    fn hash(&self) -> H256 {
        self.parcel.hash()
    }

    fn seq(&self) -> u64 {
        self.parcel.seq
    }

    fn signer_public(&self) -> Public {
        self.parcel.signer_public()
    }

    fn cost(&self) -> u64 {
        match &self.parcel.action {
            Action::Payment {
                amount,
                ..
            } => self.parcel.fee + *amount,
            Action::WrapCCC {
                amount,
                ..
            } => self.parcel.fee + *amount,
            _ => self.parcel.fee,
        }
    }
}

/// Holds parcels accessible by (signer_public, seq) and by priority
struct ParcelSet {
    by_priority: BTreeSet<ParcelOrder>,
    by_signer_public: Table<Public, u64, ParcelOrder>,
    by_fee: MultiMap<u64, H256>,
    limit: usize,
    memory_limit: usize,
}

impl ParcelSet {
    /// Inserts `ParcelOrder` to this set. Parcel does not need to be unique -
    /// the same parcel may be validly inserted twice. Any previous parcel that
    /// it replaces (i.e. with the same `signer_public` and `seq`) should be returned.
    fn insert(&mut self, signer_public: Public, seq: u64, order: ParcelOrder) -> Option<ParcelOrder> {
        if !self.by_priority.insert(order) {
            return Some(order)
        }
        let order_hash = order.hash;
        let order_fee = order.fee;
        let by_signer_public_replaced = self.by_signer_public.insert(signer_public, seq, order);
        if let Some(ref old_order) = by_signer_public_replaced {
            assert!(
                self.by_priority.remove(old_order),
                "hash is in `by_signer_public`; all parcels in `by_signer_public` must be in `by_priority`; qed"
            );
            assert!(
                self.by_fee.remove(&old_order.fee, &old_order.hash),
                "hash is in `by_signer_public`; all parcels' fee in `by_signer_public` must be in `by_fee`; qed"
            );
        }
        self.by_fee.insert(order_fee, order_hash);
        assert_eq!(self.by_priority.len(), self.by_signer_public.len());
        assert_eq!(self.by_fee.values().map(|v| v.len()).fold(0, |a, b| a + b), self.by_signer_public.len());
        by_signer_public_replaced
    }

    /// Remove low priority parcels if there is more than specified by given `limit`.
    ///
    /// It drops parecls from this set but also removes associated `VerifiedParcel`.
    /// Returns public keys and lowest seqs of parcels removed because of limit.
    fn enforce_limit(
        &mut self,
        by_hash: &mut HashMap<H256, MemPoolItem>,
        local: &mut LocalParcelsList,
    ) -> Option<HashMap<Public, u64>> {
        let mut count = 0;
        let mut mem_usage = 0;
        let to_drop: Vec<(Public, u64)> = {
            self.by_priority
                .iter()
                .filter(|order| {
                    // update parcel count and mem usage
                    count += 1;
                    mem_usage += order.mem_usage;

                    let is_own_or_retracted = order.origin.is_local() || order.origin == ParcelOrigin::RetractedBlock;
                    // Own and retracted parcels are allowed to go above all limits.
                    !is_own_or_retracted && (mem_usage > self.memory_limit || count > self.limit)
                })
                .map(|order| {
                    by_hash.get(&order.hash).expect(
                        "All parcels in `self.by_priority` and `self.by_signer_public` are kept in sync with `by_hash`.",
                    )
                })
                .map(|parcel| (parcel.signer_public(), parcel.seq()))
                .collect()
        };

        Some(to_drop.into_iter().fold(HashMap::new(), |mut removed, (sender, seq)| {
            let order = self
                .drop(&sender, seq)
                .expect("Parcel has just been found in `by_priority`; so it is in `by_signer_public` also.");
            ctrace!(MEM_POOL, "Dropped out of limit parcel: {:?}", order.hash);

            let order = by_hash
                .remove(&order.hash)
                .expect("hash is in `by_priorty`; all hashes in `by_priority` must be in `by_hash`; qed");

            if order.origin.is_local() {
                local.mark_dropped(order.parcel);
            }

            let min = removed.get(&sender).map_or(seq, |val| cmp::min(*val, seq));
            removed.insert(sender, min);
            removed
        }))
    }

    /// Drop parcel from this set (remove from `by_priority` and `by_signer_public`)
    fn drop(&mut self, signer_public: &Public, seq: u64) -> Option<ParcelOrder> {
        if let Some(parcel_order) = self.by_signer_public.remove(signer_public, &seq) {
            assert!(
                self.by_fee.remove(&parcel_order.fee, &parcel_order.hash),
                "hash is in `by_signer_public`; all parcels' fee in `by_signer_public` must be in `by_fee`; qed"
            );
            assert!(
                self.by_priority.remove(&parcel_order),
                "hash is in `by_signer_public`; all parcels in `by_signer_public` must be in `by_priority`; qed"
            );
            assert_eq!(self.by_priority.len(), self.by_signer_public.len());
            assert_eq!(self.by_fee.values().map(|v| v.len()).fold(0, |a, b| a + b), self.by_signer_public.len());
            return Some(parcel_order)
        }
        assert_eq!(self.by_priority.len(), self.by_signer_public.len());
        assert_eq!(self.by_fee.values().map(|v| v.len()).fold(0, |a, b| a + b), self.by_signer_public.len());
        None
    }

    /// Drop all parcels.
    fn clear(&mut self) {
        self.by_priority.clear();
        self.by_signer_public.clear();
    }

    /// Sets new limit for number of parcels in this `ParcelSet`.
    /// Note the limit is not applied (no parcels are removed) by calling this method.
    fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }

    /// Get the minimum fee that we can accept into this pool that wouldn't cause the parcel to
    /// immediately be dropped. 0 if the pool isn't at capacity; 1 plus the lowest if it is.
    fn fee_entry_limit(&self) -> u64 {
        match self.by_fee.keys().next() {
            Some(k) if self.by_priority.len() >= self.limit => k + 1,
            _ => 0,
        }
    }
}

pub struct MemPool {
    /// Fee threshold for parcels that can be imported to this pool (defaults to 0)
    minimal_fee: u64,
    /// Maximal time parcel may occupy the pool.
    /// When we reach `max_time_in_pool / 2^3` we re-validate
    /// account balance.
    max_time_in_pool: PoolingInstant,
    /// Priority queue for parcels that can go to block
    current: ParcelSet,
    /// Priority queue for parcels that has been received but are not yet valid to go to block
    future: ParcelSet,
    /// All parcels managed by pool indexed by hash
    by_hash: HashMap<H256, MemPoolItem>,
    /// Last seq of parcel in current (to quickly check next expected parcel)
    last_seqs: HashMap<Public, u64>,
    /// List of local parcels and their statuses.
    local_parcels: LocalParcelsList,
    /// Next id that should be assigned to a parcel imported to the pool.
    next_parcel_id: u64,
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
        let current = ParcelSet {
            by_priority: BTreeSet::new(),
            by_signer_public: Table::new(),
            by_fee: MultiMap::default(),
            limit,
            memory_limit,
        };

        let future = ParcelSet {
            by_priority: BTreeSet::new(),
            by_signer_public: Table::new(),
            by_fee: MultiMap::default(),
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
            local_parcels: LocalParcelsList::default(),
            next_parcel_id: 0,
        }
    }

    /// Set the new limit for `current` and `future` queue.
    pub fn set_limit(&mut self, limit: usize) {
        self.current.set_limit(limit);
        self.future.set_limit(limit);
        // And ensure the limits
        self.current.enforce_limit(&mut self.by_hash, &mut self.local_parcels);
        self.future.enforce_limit(&mut self.by_hash, &mut self.local_parcels);
    }

    /// Returns current limit of parcels in the pool.
    pub fn limit(&self) -> usize {
        self.current.limit
    }

    /// Get the minimal fee.
    pub fn minimal_fee(&self) -> u64 {
        self.minimal_fee
    }

    /// Sets new fee threshold for incoming parcels.
    /// Any parcel already imported to the pool is not affected.
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

    /// Add signed parcel to pool to be verified and imported.
    ///
    /// NOTE details_provider methods should be cheap to compute
    /// otherwise it might open up an attack vector.
    pub fn add<F>(
        &mut self,
        parcel: SignedParcel,
        origin: ParcelOrigin,
        time: PoolingInstant,
        timestamp: u64,
        timelock: ParcelTimelock,
        fetch_account: &F,
    ) -> Result<ParcelImportResult, ParcelError>
    where
        F: Fn(&Public) -> AccountDetails, {
        if origin == ParcelOrigin::Local {
            let hash = parcel.hash();
            let closed_parcel = parcel.clone();

            let result = self.add_internal(parcel, origin, time, timestamp, timelock, fetch_account);
            match result {
                Ok(ParcelImportResult::Current) => {
                    self.local_parcels.mark_pending(hash);
                }
                Ok(ParcelImportResult::Future) => {
                    self.local_parcels.mark_future(hash);
                }
                Err(ref err) => {
                    // Sometimes parcels are re-imported, so
                    // don't overwrite parcels if they are already on the list
                    if !self.local_parcels.contains(&hash) {
                        self.local_parcels.mark_rejected(closed_parcel, err.clone());
                    }
                }
            }
            result
        } else {
            self.add_internal(parcel, origin, time, timestamp, timelock, fetch_account)
        }
    }

    /// Checks the current seq for all parcels' senders in the pool and removes the old parcels.
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
        // Clear parcels occupying the pool too long
        let invalid = self
            .by_hash
            .iter()
            .filter(|&(_, ref parcel)| !parcel.origin.is_local())
            .map(|(hash, parcel)| (hash, parcel, current_time.saturating_sub(parcel.insertion_time)))
            .filter_map(|(hash, parcel, time_diff)| {
                if time_diff > max_time {
                    return Some(*hash)
                }

                if time_diff > balance_check {
                    return match signers.get(&parcel.signer_public()) {
                        Some(details) if parcel.cost() > details.balance => Some(*hash),
                        _ => None,
                    }
                }

                None
            })
            .collect::<Vec<_>>();
        let fetch_seq =
            |a: &Public| signers.get(a).expect("We fetch details for all signers from both current and future").seq;
        for hash in invalid {
            self.remove(&hash, &fetch_seq, RemovalReason::Invalid, current_time, timestamp);
        }
    }

    /// Removes invalid parcel identified by hash from pool.
    /// Assumption is that this parcel seq is not related to client seq,
    /// so parcels left in pool are processed according to client seq.
    ///
    /// If gap is introduced marks subsequent parcels as future
    pub fn remove<F>(
        &mut self,
        parcel_hash: &H256,
        fetch_seq: &F,
        reason: RemovalReason,
        current_time: PoolingInstant,
        timestamp: u64,
    ) where
        F: Fn(&Public) -> u64, {
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
        let parcel = self.by_hash.remove(parcel_hash);
        if parcel.is_none() {
            // We don't know this parcel
            return
        }

        let parcel = parcel.expect("None is tested in early-exit condition above; qed");
        let signer_public = parcel.signer_public();
        let seq = parcel.seq();
        let current_seq = fetch_seq(&signer_public);

        ctrace!(MEM_POOL, "Removing invalid parcel: {:?}", parcel.hash());

        // Mark in locals
        if self.local_parcels.contains(parcel_hash) {
            match reason {
                RemovalReason::Invalid => self.local_parcels.mark_invalid(parcel.parcel),
                RemovalReason::Canceled => self.local_parcels.mark_canceled(parcel.parcel),
            }
        }

        // Remove from future
        let order = self.future.drop(&signer_public, seq);
        if order.is_some() {
            self.update_future(&signer_public, current_seq);
            // And now lets check if there is some chain of parcels in future
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

    /// Removes all parcels from particular signer up to (excluding) given client (state) seq.
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

    /// Finds parcel in the pool by hash (if any)
    #[allow(dead_code)]
    pub fn find(&self, hash: &H256) -> Option<SignedParcel> {
        self.by_hash.get(hash).map(|parcel| parcel.parcel.clone())
    }

    /// Returns highest parcel seq for given signer.
    #[allow(dead_code)]
    pub fn last_seq(&self, signer_public: &Public) -> Option<u64> {
        self.last_seqs.get(signer_public).cloned()
    }

    /// Returns top parcels from the pool ordered by priority.
    pub fn top_parcels(&self, size_limit: usize) -> Vec<SignedParcel> {
        let mut current_size: usize = 0;
        self.current
            .by_priority
            .iter()
            .map(|t| {
                self.by_hash
                    .get(&t.hash)
                    .expect("All parcels in `current` and `future` are always included in `by_hash`")
            })
            .take_while(|t| {
                let encoded_byte_array: Vec<u8> = rlp::encode(&t.parcel).into_vec();
                let size_in_byte = encoded_byte_array.len();
                current_size += size_in_byte;
                current_size < size_limit
            })
            .map(|t| t.parcel.clone())
            .collect()
    }

    /// Return all future parcels.
    pub fn future_parcels(&self) -> Vec<SignedParcel> {
        self.future
            .by_priority
            .iter()
            .map(|t| {
                self.by_hash
                    .get(&t.hash)
                    .expect("All parcels in `current` and `future` are always included in `by_hash`")
            })
            .map(|t| t.parcel.clone())
            .collect()
    }

    /// Returns true if there is at least one local parcel pending
    pub fn has_local_pending_parcels(&self) -> bool {
        self.current.by_priority.iter().any(|parcel| parcel.origin == ParcelOrigin::Local)
    }

    /// Returns local parcels (some of them might not be part of the pool anymore).
    #[allow(dead_code)]
    pub fn local_parcels(&self) -> &LinkedHashMap<H256, LocalParcelStatus> {
        self.local_parcels.all_parcels()
    }

    /// Adds signed parcel to the pool.
    fn add_internal<F>(
        &mut self,
        parcel: SignedParcel,
        origin: ParcelOrigin,
        time: PoolingInstant,
        timestamp: u64,
        timelock: ParcelTimelock,
        fetch_account: &F,
    ) -> Result<ParcelImportResult, ParcelError>
    where
        F: Fn(&Public) -> AccountDetails, {
        if origin != ParcelOrigin::Local && parcel.fee < self.minimal_fee {
            ctrace!(
                MEM_POOL,
                "Dropping parcel below minimal fee: {:?} (gp: {} < {})",
                parcel.hash(),
                parcel.fee,
                self.minimal_fee
            );

            return Err(ParcelError::InsufficientFee {
                minimal: self.minimal_fee,
                got: parcel.fee,
            })
        }

        let full_pools_lowest = self.effective_minimum_fee();
        if parcel.fee < full_pools_lowest && origin != ParcelOrigin::Local {
            ctrace!(
                MEM_POOL,
                "Dropping parcel below lowest fee in a full pool: {:?} (gp: {} < {})",
                parcel.hash(),
                parcel.fee,
                full_pools_lowest
            );

            return Err(ParcelError::InsufficientFee {
                minimal: full_pools_lowest,
                got: parcel.fee,
            })
        }

        let client_account = fetch_account(&parcel.signer_public());
        if client_account.balance < parcel.fee {
            ctrace!(
                MEM_POOL,
                "Dropping parcel without sufficient balance: {:?} ({} < {})",
                parcel.hash(),
                client_account.balance,
                parcel.fee
            );

            return Err(ParcelError::InsufficientBalance {
                address: public_to_address(&parcel.signer_public()),
                cost: parcel.fee,
                balance: client_account.balance,
            })
        }
        parcel.check_low_s()?;
        // No invalid parcels beyond this point.
        let id = self.next_parcel_id;
        self.next_parcel_id += 1;
        let vparcel = MemPoolItem::new(parcel, origin, time, id, timelock);
        let r = self.import_parcel(vparcel, client_account.seq, timestamp);
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
        r
    }

    /// Adds VerifiedParcel to this pool.
    ///
    /// Determines if it should be placed in current or future. When parcel is
    /// imported to `current` also checks if there are any `future` parcels that should be promoted because of
    /// this.
    ///
    /// It ignores parcels that has already been imported (same `hash`) and replaces the parcel
    /// iff `(address, seq)` is the same but `fee` is higher.
    ///
    /// Returns `true` when parcel was imported successfully
    fn import_parcel(
        &mut self,
        parcel: MemPoolItem,
        state_seq: u64,
        timestamp: u64,
    ) -> Result<ParcelImportResult, ParcelError> {
        if self.by_hash.get(&parcel.hash()).is_some() {
            // Parcel is already imported.
            ctrace!(MEM_POOL, "Dropping already imported parcel: {:?}", parcel.hash());
            return Err(ParcelError::ParcelAlreadyImported)
        }

        let signer_public = parcel.signer_public();
        let seq = parcel.seq();
        let hash = parcel.hash();

        // The parcel might be old, let's check that.
        // This has to be the first test, otherwise calculating
        // seq height would result in overflow.
        if seq < state_seq {
            // Droping parcel
            ctrace!(MEM_POOL, "Dropping old parcel: {:?} (seq: {} < {})", parcel.hash(), seq, state_seq);
            return Err(ParcelError::Old)
        }

        // Update seqs of parcels in future (remove old parcels)
        self.update_future(&signer_public, state_seq);
        // State seq could be updated. Maybe there are some more items waiting in future?
        self.move_matching_future_to_current(signer_public, state_seq, state_seq, parcel.insertion_time, timestamp);
        // Check the next expected seq (might be updated by move above)
        let next_seq = self.last_seqs.get(&signer_public).map_or(state_seq, |n| *n + 1);

        if parcel.origin.is_local() {
            self.mark_parcels_local(&signer_public);
        }

        // Future parcel
        if seq > next_seq {
            // We have a gap - put to future.
            // Insert parcel (or replace old one with lower fee)
            check_too_cheap(Self::replace_parcel(
                parcel,
                state_seq,
                &mut self.future,
                &mut self.by_hash,
                &mut self.local_parcels,
            ))?;
            // Enforce limit in Future
            let removed = self.future.enforce_limit(&mut self.by_hash, &mut self.local_parcels);
            // Return an error if this parcel was not imported because of limit.
            check_if_removed(&signer_public, seq, removed)?;

            cdebug!(MEM_POOL, "Importing parcel to future: {:?}", hash);
            cdebug!(MEM_POOL, "status: {:?}", self.status());
            return Ok(ParcelImportResult::Future)
        }

        // We might have filled a gap - move some more parcels from future
        self.move_matching_future_to_current(signer_public, seq, state_seq, parcel.insertion_time, timestamp);
        self.move_matching_future_to_current(signer_public, seq + 1, state_seq, parcel.insertion_time, timestamp);

        if Self::should_wait_timelock(&parcel.timelock, parcel.insertion_time, timestamp) {
            // Check same seq is in current. If it
            // is than move the following current items to future.
            let best_block_number = parcel.insertion_time;
            let moved_to_future_flag = self.current.by_signer_public.get(&signer_public, &seq).is_some();
            if moved_to_future_flag {
                self.move_all_to_future(&signer_public, state_seq);
            }

            check_too_cheap(Self::replace_parcel(
                parcel,
                state_seq,
                &mut self.future,
                &mut self.by_hash,
                &mut self.local_parcels,
            ))?;

            if moved_to_future_flag {
                self.move_matching_future_to_current(signer_public, state_seq, state_seq, best_block_number, timestamp);
            }

            let removed = self.future.enforce_limit(&mut self.by_hash, &mut self.local_parcels);
            check_if_removed(&signer_public, seq, removed)?;
            cdebug!(MEM_POOL, "Imported parcel to future: {:?}", hash);
            cdebug!(MEM_POOL, "status: {:?}", self.status());
            return Ok(ParcelImportResult::Future)
        }

        // Replace parcel if any
        check_too_cheap(Self::replace_parcel(
            parcel,
            state_seq,
            &mut self.current,
            &mut self.by_hash,
            &mut self.local_parcels,
        ))?;
        // Keep track of highest seq stored in current
        let new_max = self.last_seqs.get(&signer_public).map_or(seq, |n| cmp::max(seq, *n));
        self.last_seqs.insert(signer_public, new_max);

        // Also enforce the limit
        let removed = self.current.enforce_limit(&mut self.by_hash, &mut self.local_parcels);
        // If some parcel were removed because of limit we need to update last_seqs also.
        self.update_last_seqs(&removed);
        // Trigger error if the parcel we are importing was removed.
        check_if_removed(&signer_public, seq, removed)?;

        cdebug!(MEM_POOL, "Imported parcel to current: {:?}", hash);
        cdebug!(MEM_POOL, "status: {:?}", self.status());
        Ok(ParcelImportResult::Current)
    }

    fn should_wait_timelock(
        timelock: &ParcelTimelock,
        best_block_number: BlockNumber,
        best_block_timestamp: u64,
    ) -> bool {
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

    /// Always updates future and moves parcel from current to future.
    fn cull_internal(&mut self, sender: Public, client_seq: u64, current_time: PoolingInstant, timestamp: u64) {
        // We will either move parcel to future or remove it completely
        // so there will be no parcels from this sender in current
        self.last_seqs.remove(&sender);
        // First update height of parcels in future to avoid collisions
        self.update_future(&sender, client_seq);
        // This should move all current parcels to future and remove old parcels
        self.move_all_to_future(&sender, client_seq);
        // And now lets check if there is some batch of parcels in future
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

    /// Update height of all parcels in future parcels set.
    fn update_future(&mut self, signer_public: &Public, current_seq: u64) {
        // We need to drain all parcels for current signer from future and reinsert them with updated height
        let all_seqs_from_sender = match self.future.by_signer_public.row(signer_public) {
            Some(row_map) => row_map.keys().cloned().collect::<Vec<_>>(),
            None => vec![],
        };
        for k in all_seqs_from_sender {
            let order = self
                .future
                .drop(signer_public, k)
                .expect("iterating over a collection that has been retrieved above; qed");
            if k >= current_seq {
                self.future.insert(*signer_public, k, order.update_height(k, current_seq));
            } else {
                ctrace!(MEM_POOL, "Removing old parcel: {:?} (seq: {} < {})", order.hash, k, current_seq);
                // Remove the parcel completely
                self.by_hash.remove(&order.hash).expect("All parcels in `future` are also in `by_hash`");
            }
        }
    }

    /// Checks if there are any parcels in `future` that should actually be promoted to `current`
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
            let by_seq = self.future.by_signer_public.row_mut(&public);
            if by_seq.is_none() {
                return
            }
            let by_seq = by_seq.expect("None is tested in early-exit condition above; qed");
            while let Some(order) = by_seq.get(&current_seq).cloned() {
                if Self::should_wait_timelock(&order.timelock, best_block_number, best_block_timestamp) {
                    break
                }
                let order = by_seq.remove(&current_seq).expect("None is tested in the while condition above.");
                self.future.by_priority.remove(&order);
                self.future.by_fee.remove(&order.fee, &order.hash);
                // Put to current
                let order = order.update_height(current_seq, first_seq);
                if order.origin.is_local() {
                    self.local_parcels.mark_pending(order.hash);
                }
                if let Some(old) = self.current.insert(public, current_seq, order) {
                    Self::replace_orders(
                        public,
                        current_seq,
                        old,
                        order,
                        &mut self.current,
                        &mut self.by_hash,
                        &mut self.local_parcels,
                    );
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

    /// Drop all parcels from given signer from `current`.
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
                if order.origin.is_local() {
                    self.local_parcels.mark_future(order.hash);
                }
                if let Some(old) = self.future.insert(*signer_public, k, order) {
                    Self::replace_orders(
                        *signer_public,
                        k,
                        old,
                        order,
                        &mut self.future,
                        &mut self.by_hash,
                        &mut self.local_parcels,
                    );
                }
            } else {
                ctrace!(MEM_POOL, "Removing old parcel: {:?} (seq: {} < {})", order.hash, k, current_seq);
                let parcel = self.by_hash.remove(&order.hash).expect("All parcels in `future` are also in `by_hash`");
                if parcel.origin.is_local() {
                    self.local_parcels.mark_mined(parcel.parcel);
                }
            }
        }
        self.future.enforce_limit(&mut self.by_hash, &mut self.local_parcels);
    }

    /// Marks all parcels from particular sender as local parcels
    fn mark_parcels_local(&mut self, signer: &Public) {
        fn mark_local<F: FnMut(H256)>(signer_public: &Public, set: &mut ParcelSet, mut mark: F) {
            // Mark all parcels from this signer as local
            let seqs_from_sender = set
                .by_signer_public
                .row(signer_public)
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
                    set.drop(signer_public, k).expect("parcel known to be in self.current/self.future; qed");
                order.origin = ParcelOrigin::Local;
                mark(order.hash);
                set.insert(*signer_public, k, order);
            }
        }

        let local = &mut self.local_parcels;
        mark_local(signer, &mut self.current, |hash| local.mark_pending(hash));
        mark_local(signer, &mut self.future, |hash| local.mark_future(hash));
    }

    /// Replaces parcel in given set (could be `future` or `current`).
    ///
    /// If there is already parcel with same `(sender, seq)` it will be replaced iff `fee` is higher.
    /// One of the parcels is dropped from set and also removed from pool entirely (from `by_hash`).
    ///
    /// Returns `true` if parcel actually got to the pool (`false` if there was already a parcel with higher
    /// fee)
    fn replace_parcel(
        parcel: MemPoolItem,
        base_seq: u64,
        set: &mut ParcelSet,
        by_hash: &mut HashMap<H256, MemPoolItem>,
        local: &mut LocalParcelsList,
    ) -> bool {
        let order = ParcelOrder::for_parcel(&parcel, base_seq);
        let hash = parcel.hash();
        let signer_public = parcel.signer_public();
        let seq = parcel.seq();

        let old_hash = by_hash.insert(hash, parcel);
        assert!(old_hash.is_none(), "Each hash has to be inserted exactly once.");

        ctrace!(MEM_POOL, "Inserting: {:?}", order);

        if let Some(old) = set.insert(signer_public, seq, order) {
            Self::replace_orders(signer_public, seq, old, order, set, by_hash, local)
        } else {
            true
        }
    }

    fn replace_orders(
        signer_public: Public,
        seq: u64,
        old: ParcelOrder,
        order: ParcelOrder,
        set: &mut ParcelSet,
        by_hash: &mut HashMap<H256, MemPoolItem>,
        local: &mut LocalParcelsList,
    ) -> bool {
        // There was already parcel in pool. Let's check which one should stay
        let old_hash = old.hash;
        let new_hash = order.hash;

        let old_fee = old.fee;
        let new_fee = order.fee;
        let min_required_fee = old_fee + (old_fee >> FEE_BUMP_SHIFT);

        if min_required_fee > new_fee {
            ctrace!(
                MEM_POOL,
                "Didn't insert parcel because fee was too low: {:?} ({:?} stays in the pool)",
                order.hash,
                old.hash
            );
            // Put back old parcel since it has greater priority (higher fee)
            set.insert(signer_public, seq, old);
            // and remove new one
            let order = by_hash
                .remove(&order.hash)
                .expect("The hash has been just inserted and no other line is altering `by_hash`.");
            if order.origin.is_local() {
                local.mark_replaced(order.parcel, old_fee, old_hash);
            }
            false
        } else {
            ctrace!(MEM_POOL, "Replaced parcel: {:?} with parcel with higher fee: {:?}", old.hash, order.hash);
            // Make sure we remove old parcel entirely
            let old =
                by_hash.remove(&old.hash).expect("The hash is coming from `future` so it has to be in `by_hash`.");
            if old.origin.is_local() {
                local.mark_replaced(old.parcel, new_fee, new_hash);
            }
            true
        }
    }
}

#[derive(Debug)]
/// Current status of the pool
pub struct MemPoolStatus {
    /// Number of pending parcels (ready to go to block)
    pub pending: usize,
    /// Number of future parcels (waiting for parcels with lower seqs first)
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

/// Reason to remove single parcel from the pool.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RemovalReason {
    /// Parcel is invalid
    Invalid,
    /// Parcel was canceled
    #[allow(dead_code)]
    Canceled,
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
    use ctypes::parcel::Parcel;
    use ctypes::transaction::{AssetMintOutput, Transaction};
    use primitives::H160;

    use super::*;

    #[test]
    fn parcel_origin_ordering() {
        assert_eq!(ParcelOrigin::Local.cmp(&ParcelOrigin::External), Ordering::Less);
        assert_eq!(ParcelOrigin::RetractedBlock.cmp(&ParcelOrigin::Local), Ordering::Less);
        assert_eq!(ParcelOrigin::RetractedBlock.cmp(&ParcelOrigin::External), Ordering::Less);

        assert_eq!(ParcelOrigin::External.cmp(&ParcelOrigin::Local), Ordering::Greater);
        assert_eq!(ParcelOrigin::Local.cmp(&ParcelOrigin::RetractedBlock), Ordering::Greater);
        assert_eq!(ParcelOrigin::External.cmp(&ParcelOrigin::RetractedBlock), Ordering::Greater);
    }

    #[test]
    fn parcel_timelock_ordering() {
        assert_eq!(
            ParcelTimelock {
                block: None,
                timestamp: None
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: None
            }),
            Ordering::Less
        );
        assert_eq!(
            ParcelTimelock {
                block: None,
                timestamp: None
            }
            .cmp(&ParcelTimelock {
                block: None,
                timestamp: Some(100)
            }),
            Ordering::Less
        );

        // Block is the prior condition.
        assert_eq!(
            ParcelTimelock {
                block: Some(9),
                timestamp: None
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: None
            }),
            Ordering::Less
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(9),
                timestamp: None
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(9),
                timestamp: Some(100)
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: None
            }),
            Ordering::Less
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(9),
                timestamp: Some(99)
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(9),
                timestamp: Some(101)
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(11),
                timestamp: None
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: None
            }),
            Ordering::Greater
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(11),
                timestamp: None
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Greater
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(11),
                timestamp: Some(100)
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: None
            }),
            Ordering::Greater
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(11),
                timestamp: Some(99)
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Greater
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(11),
                timestamp: Some(101)
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Greater
        );

        // Compare timestamp if blocks are equal.
        assert_eq!(
            ParcelTimelock {
                block: Some(10),
                timestamp: None
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(10),
                timestamp: Some(99)
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Equal
        );
        assert_eq!(
            ParcelTimelock {
                block: Some(10),
                timestamp: Some(101)
            }
            .cmp(&ParcelTimelock {
                block: Some(10),
                timestamp: Some(100)
            }),
            Ordering::Greater
        );
        assert_eq!(
            ParcelTimelock {
                block: None,
                timestamp: None
            }
            .cmp(&ParcelTimelock {
                block: None,
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            ParcelTimelock {
                block: None,
                timestamp: Some(99)
            }
            .cmp(&ParcelTimelock {
                block: None,
                timestamp: Some(100)
            }),
            Ordering::Less
        );
        assert_eq!(
            ParcelTimelock {
                block: None,
                timestamp: Some(100)
            }
            .cmp(&ParcelTimelock {
                block: None,
                timestamp: Some(100)
            }),
            Ordering::Equal
        );
        assert_eq!(
            ParcelTimelock {
                block: None,
                timestamp: Some(101)
            }
            .cmp(&ParcelTimelock {
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
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: "Metadata".to_string(),
            output: AssetMintOutput {
                lock_script_hash: H160::zero(),
                parameters: vec![],
                amount: None,
            },
            registrar: None,
        };
        let parcel = Parcel {
            seq: 0,
            fee,
            network_id: "tc".into(),
            action: Action::AssetTransaction(transaction),
        };
        let timelock = ParcelTimelock {
            block: None,
            timestamp: None,
        };
        let keypair = Random.generate().unwrap();
        let signed = SignedParcel::new_with_sign(parcel, keypair.private());
        let item = MemPoolItem::new(signed, ParcelOrigin::Local, 0, 0, timelock);

        assert_eq!(fee, item.cost());
    }

    #[test]
    fn transfer_transaction_does_not_increase_cost() {
        let fee = 100;
        let transaction = Transaction::AssetTransfer {
            network_id: "tc".into(),
            burns: vec![],
            inputs: vec![],
            outputs: vec![],
        };
        let parcel = Parcel {
            seq: 0,
            fee,
            network_id: "tc".into(),
            action: Action::AssetTransaction(transaction),
        };
        let timelock = ParcelTimelock {
            block: None,
            timestamp: None,
        };
        let keypair = Random.generate().unwrap();
        let signed = SignedParcel::new_with_sign(parcel, keypair.private());
        let item = MemPoolItem::new(signed, ParcelOrigin::Local, 0, 0, timelock);

        assert_eq!(fee, item.cost());
    }

    #[test]
    fn payment_increases_cost() {
        let fee = 100;
        let amount = 100000;
        let receiver = 1u64.into();
        let keypair = Random.generate().unwrap();
        let parcel = Parcel {
            seq: 0,
            fee,
            network_id: "tc".into(),
            action: Action::Payment {
                receiver,
                amount,
            },
        };
        let timelock = ParcelTimelock {
            block: None,
            timestamp: None,
        };
        let signed = SignedParcel::new_with_sign(parcel, keypair.private());
        let item = MemPoolItem::new(signed, ParcelOrigin::Local, 0, 0, timelock);

        assert_eq!(fee + amount, item.cost());
    }

    #[test]
    fn fee_per_byte_order_simple() {
        let order1 = create_parcel_order(1000_000_000, 100);
        let order2 = create_parcel_order(1500_000_000, 300);
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
        let mut orders: Vec<ParcelOrder> = Vec::new();
        for factor in factors {
            let fee = 1000_000 * (factor[0] as u64);
            orders.push(create_parcel_order(fee, 10 * factor[1]));
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

    fn create_parcel_order(fee: u64, transaction_count: usize) -> ParcelOrder {
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id: 0,
            metadata: String::from_utf8(vec!['a' as u8; transaction_count]).unwrap(),
            registrar: None,
            output: AssetMintOutput {
                lock_script_hash: H160::zero(),
                parameters: vec![],
                amount: None,
            },
        };
        let keypair = Random.generate().unwrap();
        let parcel = Parcel {
            seq: 0,
            fee,
            network_id: "tc".into(),
            action: Action::AssetTransaction(transaction),
        };
        let timelock = ParcelTimelock {
            block: None,
            timestamp: None,
        };
        let signed = SignedParcel::new_with_sign(parcel, keypair.private());
        let item = MemPoolItem::new(signed, ParcelOrigin::Local, 0, 0, timelock);
        ParcelOrder::for_parcel(&item, 0)
    }
}

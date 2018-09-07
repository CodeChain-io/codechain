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
use primitives::{H256, U256};
use rlp;
use table::Table;

use super::super::parcel::SignedParcel;
use super::local_parcels::{LocalParcelsList, Status as LocalParcelStatus};
use super::ParcelImportResult;

/// Parcel with the same (sender, nonce) can be replaced only if
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
    fn is_local(&self) -> bool {
        *self == ParcelOrigin::Local
    }
}

#[derive(Clone, Debug)]
/// Light structure used to identify parcel and its order
struct ParcelOrder {
    /// Primary ordering factory. Difference between parcel nonce and expected nonce in state
    /// (e.g. Parcel(nonce:5), State(nonce:0) -> height: 5)
    /// High nonce_height = Low priority (processed later)
    nonce_height: U256,
    /// Fee of the parcel.
    fee: U256,
    /// Fee per bytes(rlp serialized) of the parcel
    fee_per_byte: U256,
    /// Heap usage of this parcel.
    mem_usage: usize,
    /// Hash to identify associated parcel
    hash: H256,
    /// Incremental id assigned when parcel is inserted to the pool.
    insertion_id: u64,
    /// Origin of the parcel
    origin: ParcelOrigin,
}

impl ParcelOrder {
    fn for_parcel(item: &MemPoolItem, base_nonce: U256) -> Self {
        let rlp_bytes_len = rlp::encode(&item.parcel).to_vec().len();
        let fee = item.parcel.fee;
        ctrace!(MEM_POOL, "New parcel with size {}", item.parcel.heap_size_of_children());
        Self {
            nonce_height: item.nonce() - base_nonce,
            fee,
            mem_usage: item.parcel.heap_size_of_children(),
            fee_per_byte: fee / rlp_bytes_len.into(),
            hash: item.hash(),
            insertion_id: item.insertion_id,
            origin: item.origin,
        }
    }

    fn update_height(mut self, nonce: U256, base_nonce: U256) -> Self {
        self.nonce_height = nonce - base_nonce;
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

        // Check nonce_height
        if self.nonce_height != b.nonce_height {
            return self.nonce_height.cmp(&b.nonce_height)
        }

        if self.fee_per_byte != b.fee_per_byte {
            return self.fee_per_byte.cmp(&b.fee_per_byte)
        }

        // Then compare fee
        if self.fee != b.fee {
            return b.fee.cmp(&self.fee)
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
}

impl MemPoolItem {
    fn new(parcel: SignedParcel, origin: ParcelOrigin, insertion_time: PoolingInstant, insertion_id: u64) -> Self {
        MemPoolItem {
            parcel,
            origin,
            insertion_time,
            insertion_id,
        }
    }

    fn hash(&self) -> H256 {
        self.parcel.hash()
    }

    fn nonce(&self) -> U256 {
        self.parcel.nonce
    }

    fn signer_public(&self) -> Public {
        self.parcel.signer_public()
    }

    fn cost(&self) -> U256 {
        match &self.parcel.action {
            Action::Payment {
                amount,
                ..
            } => self.parcel.fee + *amount,
            _ => self.parcel.fee,
        }
    }
}

/// Holds parcels accessible by (signer_public, nonce) and by priority
struct ParcelSet {
    by_priority: BTreeSet<ParcelOrder>,
    by_signer_public: Table<Public, U256, ParcelOrder>,
    by_fee: MultiMap<U256, H256>,
    limit: usize,
    memory_limit: usize,
}

impl ParcelSet {
    /// Inserts `ParcelOrder` to this set. Parcel does not need to be unique -
    /// the same parcel may be validly inserted twice. Any previous parcel that
    /// it replaces (i.e. with the same `signer_public` and `nonce`) should be returned.
    fn insert(&mut self, signer_public: Public, nonce: U256, order: ParcelOrder) -> Option<ParcelOrder> {
        if !self.by_priority.insert(order.clone()) {
            return Some(order.clone())
        }
        let order_hash = order.hash.clone();
        let order_fee = order.fee.clone();
        let by_signer_public_replaced = self.by_signer_public.insert(signer_public, nonce, order);
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
    /// Returns public keys and lowest nonces of parcels removed because of limit.
    fn enforce_limit(
        &mut self,
        by_hash: &mut HashMap<H256, MemPoolItem>,
        local: &mut LocalParcelsList,
    ) -> Option<HashMap<Public, U256>> {
        let mut count = 0;
        let mut mem_usage = 0;
        let to_drop: Vec<(Public, U256)> = {
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
                .map(|parcel| (parcel.signer_public(), parcel.nonce()))
                .collect()
        };

        Some(to_drop.into_iter().fold(HashMap::new(), |mut removed, (sender, nonce)| {
            let order = self
                .drop(&sender, &nonce)
                .expect("Parcel has just been found in `by_priority`; so it is in `by_signer_public` also.");
            ctrace!(MEM_POOL, "Dropped out of limit parcel: {:?}", order.hash);

            let order = by_hash
                .remove(&order.hash)
                .expect("hash is in `by_priorty`; all hashes in `by_priority` must be in `by_hash`; qed");

            if order.origin.is_local() {
                local.mark_dropped(order.parcel);
            }

            let min = removed.get(&sender).map_or(nonce, |val| cmp::min(*val, nonce));
            removed.insert(sender, min);
            removed
        }))
    }

    /// Drop parcel from this set (remove from `by_priority` and `by_signer_public`)
    fn drop(&mut self, signer_public: &Public, nonce: &U256) -> Option<ParcelOrder> {
        if let Some(parcel_order) = self.by_signer_public.remove(signer_public, nonce) {
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
    #[allow(dead_code)]
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
    fn fee_entry_limit(&self) -> U256 {
        match self.by_fee.keys().next() {
            Some(k) if self.by_priority.len() >= self.limit => *k + 1.into(),
            _ => U256::default(),
        }
    }
}

pub struct MemPool {
    /// Fee threshold for parcels that can be imported to this pool (defaults to 0)
    minimal_fee: U256,
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
    /// Last nonce of parcel in current (to quickly check next expected parcel)
    last_nonces: HashMap<Public, U256>,
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
            minimal_fee: U256::zero(),
            max_time_in_pool: DEFAULT_POOLING_PERIOD,
            current,
            future,
            by_hash: HashMap::new(),
            last_nonces: HashMap::new(),
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
    pub fn minimal_fee(&self) -> &U256 {
        &self.minimal_fee
    }

    /// Sets new fee threshold for incoming parcels.
    /// Any parcel already imported to the pool is not affected.
    pub fn set_minimal_fee(&mut self, min_fee: U256) {
        self.minimal_fee = min_fee;
    }

    /// Get one more than the lowest fee in the pool iff the pool is
    /// full, otherwise 0.
    pub fn effective_minimum_fee(&self) -> U256 {
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
        fetch_account: &F,
    ) -> Result<ParcelImportResult, ParcelError>
    where
        F: Fn(&Public) -> AccountDetails, {
        if origin == ParcelOrigin::Local {
            let hash = parcel.hash();
            let closed_parcel = parcel.clone();

            let result = self.add_internal(parcel, origin, time, fetch_account);
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
            self.add_internal(parcel, origin, time, fetch_account)
        }
    }

    /// Checks the current nonce for all parcels' senders in the pool and removes the old parcels.
    pub fn remove_old<F>(&mut self, fetch_account: &F, current_time: PoolingInstant)
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
            self.cull(*signer, details.nonce);
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
        let fetch_nonce =
            |a: &Public| signers.get(a).expect("We fetch details for all signers from both current and future").nonce;
        for hash in invalid {
            self.remove(&hash, &fetch_nonce, RemovalReason::Invalid);
        }
    }

    /// Removes invalid parcel identified by hash from pool.
    /// Assumption is that this parcel nonce is not related to client nonce,
    /// so parcels left in pool are processed according to client nonce.
    ///
    /// If gap is introduced marks subsequent parcels as future
    pub fn remove<F>(&mut self, parcel_hash: &H256, fetch_nonce: &F, reason: RemovalReason)
    where
        F: Fn(&Public) -> U256, {
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
        let parcel = self.by_hash.remove(parcel_hash);
        if parcel.is_none() {
            // We don't know this parcel
            return
        }

        let parcel = parcel.expect("None is tested in early-exit condition above; qed");
        let signer_public = parcel.signer_public();
        let nonce = parcel.nonce();
        let current_nonce = fetch_nonce(&signer_public);

        ctrace!(MEM_POOL, "Removing invalid parcel: {:?}", parcel.hash());

        // Mark in locals
        if self.local_parcels.contains(parcel_hash) {
            match reason {
                RemovalReason::Invalid => self.local_parcels.mark_invalid(parcel.parcel.into()),
                RemovalReason::Canceled => self.local_parcels.mark_canceled(parcel.parcel.into()),
            }
        }

        // Remove from future
        let order = self.future.drop(&signer_public, &nonce);
        if order.is_some() {
            self.update_future(&signer_public, current_nonce);
            // And now lets check if there is some chain of parcels in future
            // that should be placed in current
            self.move_matching_future_to_current(signer_public, current_nonce, current_nonce);
            assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
            return
        }

        // Remove from current
        let order = self.current.drop(&signer_public, &nonce);
        if order.is_some() {
            // This will keep consistency in pool
            // Moves all to future and then promotes a batch from current:
            self.cull_internal(signer_public, current_nonce);
            assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
            return
        }
    }

    /// Removes all parcels from particular signer up to (excluding) given client (state) nonce.
    /// Client (State) Nonce = next valid nonce for this signer.
    pub fn cull(&mut self, signer_public: Public, client_nonce: U256) {
        // Check if there is anything in current...
        let should_check_in_current = self.current.by_signer_public.row(&signer_public)
            // If nonce == client_nonce nothing is changed
            .and_then(|by_nonce| by_nonce.keys().find(|nonce| *nonce < &client_nonce))
            .map(|_| ());
        // ... or future
        let should_check_in_future = self.future.by_signer_public.row(&signer_public)
            // if nonce == client_nonce we need to promote to current
            .and_then(|by_nonce| by_nonce.keys().find(|nonce| *nonce <= &client_nonce))
            .map(|_| ());

        if should_check_in_current.or(should_check_in_future).is_none() {
            return
        }

        self.cull_internal(signer_public, client_nonce);
    }

    /// Removes all elements (in any state) from the pool
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.current.clear();
        self.future.clear();
        self.by_hash.clear();
        self.last_nonces.clear();
    }

    /// Finds parcel in the pool by hash (if any)
    #[allow(dead_code)]
    pub fn find(&self, hash: &H256) -> Option<SignedParcel> {
        self.by_hash.get(hash).map(|parcel| parcel.parcel.clone())
    }

    /// Returns highest parcel nonce for given signer.
    #[allow(dead_code)]
    pub fn last_nonce(&self, signer_public: &Public) -> Option<U256> {
        self.last_nonces.get(signer_public).cloned()
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
                return current_size < size_limit
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
        let vparcel = MemPoolItem::new(parcel, origin, time, id);
        let r = self.import_parcel(vparcel, client_account.nonce);
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
    /// iff `(address, nonce)` is the same but `fee` is higher.
    ///
    /// Returns `true` when parcel was imported successfully
    fn import_parcel(&mut self, parcel: MemPoolItem, state_nonce: U256) -> Result<ParcelImportResult, ParcelError> {
        if self.by_hash.get(&parcel.hash()).is_some() {
            // Parcel is already imported.
            ctrace!(MEM_POOL, "Dropping already imported parcel: {:?}", parcel.hash());
            return Err(ParcelError::ParcelAlreadyImported)
        }

        let signer_public = parcel.signer_public();
        let nonce = parcel.nonce();
        let hash = parcel.hash();

        // The parcel might be old, let's check that.
        // This has to be the first test, otherwise calculating
        // nonce height would result in overflow.
        if nonce < state_nonce {
            // Droping parcel
            ctrace!(MEM_POOL, "Dropping old parcel: {:?} (nonce: {} < {})", parcel.hash(), nonce, state_nonce);
            return Err(ParcelError::Old)
        }

        // Update nonces of parcels in future (remove old parcels)
        self.update_future(&signer_public, state_nonce);
        // State nonce could be updated. Maybe there are some more items waiting in future?
        self.move_matching_future_to_current(signer_public, state_nonce, state_nonce);
        // Check the next expected nonce (might be updated by move above)
        let next_nonce = self.last_nonces.get(&signer_public).cloned().map_or(state_nonce, |n| n + U256::one());

        if parcel.origin.is_local() {
            self.mark_parcels_local(&signer_public);
        }

        // Future parcel
        if nonce > next_nonce {
            // We have a gap - put to future.
            // Insert parcel (or replace old one with lower fee)
            check_too_cheap(Self::replace_parcel(
                parcel,
                state_nonce,
                &mut self.future,
                &mut self.by_hash,
                &mut self.local_parcels,
            ))?;
            // Enforce limit in Future
            let removed = self.future.enforce_limit(&mut self.by_hash, &mut self.local_parcels);
            // Return an error if this parcel was not imported because of limit.
            check_if_removed(&signer_public, &nonce, removed)?;

            cdebug!(MEM_POOL, "Importing parcel to future: {:?}", hash);
            cdebug!(MEM_POOL, "status: {:?}", self.status());
            return Ok(ParcelImportResult::Future)
        }

        // We might have filled a gap - move some more parcels from future
        self.move_matching_future_to_current(signer_public, nonce, state_nonce);
        self.move_matching_future_to_current(signer_public, nonce + U256::one(), state_nonce);

        // Replace parcel if any
        check_too_cheap(Self::replace_parcel(
            parcel,
            state_nonce,
            &mut self.current,
            &mut self.by_hash,
            &mut self.local_parcels,
        ))?;
        // Keep track of highest nonce stored in current
        let new_max = self.last_nonces.get(&signer_public).map_or(nonce, |n| cmp::max(nonce, *n));
        self.last_nonces.insert(signer_public, new_max);

        // Also enforce the limit
        let removed = self.current.enforce_limit(&mut self.by_hash, &mut self.local_parcels);
        // If some parcel were removed because of limit we need to update last_nonces also.
        self.update_last_nonces(&removed);
        // Trigger error if the parcel we are importing was removed.
        check_if_removed(&signer_public, &nonce, removed)?;

        cdebug!(MEM_POOL, "Imported parcel to current: {:?}", hash);
        cdebug!(MEM_POOL, "status: {:?}", self.status());
        Ok(ParcelImportResult::Current)
    }

    /// Always updates future and moves parcel from current to future.
    fn cull_internal(&mut self, sender: Public, client_nonce: U256) {
        // We will either move parcel to future or remove it completely
        // so there will be no parcels from this sender in current
        self.last_nonces.remove(&sender);
        // First update height of parcels in future to avoid collisions
        self.update_future(&sender, client_nonce);
        // This should move all current parcels to future and remove old parcels
        self.move_all_to_future(&sender, client_nonce);
        // And now lets check if there is some batch of parcels in future
        // that should be placed in current. It should also update last_nonces.
        self.move_matching_future_to_current(sender, client_nonce, client_nonce);
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
    }

    fn update_last_nonces(&mut self, removed_min_nonces: &Option<HashMap<Public, U256>>) {
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

    /// Update height of all parcels in future parcels set.
    fn update_future(&mut self, signer_public: &Public, current_nonce: U256) {
        // We need to drain all parcels for current signer from future and reinsert them with updated height
        let all_nonces_from_sender = match self.future.by_signer_public.row(signer_public) {
            Some(row_map) => row_map.keys().cloned().collect::<Vec<U256>>(),
            None => vec![],
        };
        for k in all_nonces_from_sender {
            let order = self
                .future
                .drop(signer_public, &k)
                .expect("iterating over a collection that has been retrieved above; qed");
            if k >= current_nonce {
                self.future.insert(*signer_public, k, order.update_height(k, current_nonce));
            } else {
                ctrace!(MEM_POOL, "Removing old parcel: {:?} (nonce: {} < {})", order.hash, k, current_nonce);
                // Remove the parcel completely
                self.by_hash.remove(&order.hash).expect("All parcels in `future` are also in `by_hash`");
            }
        }
    }

    /// Checks if there are any parcels in `future` that should actually be promoted to `current`
    /// (because nonce matches).
    fn move_matching_future_to_current(&mut self, public: Public, mut current_nonce: U256, first_nonce: U256) {
        let mut update_last_nonce_to = None;
        {
            let by_nonce = self.future.by_signer_public.row_mut(&public);
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
                    self.local_parcels.mark_pending(order.hash);
                }
                if let Some(old) = self.current.insert(public, current_nonce, order.clone()) {
                    Self::replace_orders(
                        public,
                        current_nonce,
                        old,
                        order,
                        &mut self.current,
                        &mut self.by_hash,
                        &mut self.local_parcels,
                    );
                }
                update_last_nonce_to = Some(current_nonce);
                current_nonce = current_nonce + U256::one();
            }
        }
        self.future.by_signer_public.clear_if_empty(&public);
        if let Some(x) = update_last_nonce_to {
            // Update last inserted nonce
            self.last_nonces.insert(public, x);
        }
    }

    /// Drop all parcels from given signer from `current`.
    /// Either moves them to `future` or removes them from pool completely.
    fn move_all_to_future(&mut self, signer_public: &Public, current_nonce: U256) {
        let all_nonces_from_sender = match self.current.by_signer_public.row(signer_public) {
            Some(row_map) => row_map.keys().cloned().collect::<Vec<U256>>(),
            None => vec![],
        };

        for k in all_nonces_from_sender {
            // Goes to future or is removed
            let order = self
                .current
                .drop(signer_public, &k)
                .expect("iterating over a collection that has been retrieved above; qed");
            if k >= current_nonce {
                let order = order.update_height(k, current_nonce);
                if order.origin.is_local() {
                    self.local_parcels.mark_future(order.hash);
                }
                if let Some(old) = self.future.insert(*signer_public, k, order.clone()) {
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
                ctrace!(MEM_POOL, "Removing old parcel: {:?} (nonce: {} < {})", order.hash, k, current_nonce);
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
            let nonces_from_sender = set
                .by_signer_public
                .row(signer_public)
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
                let mut order =
                    set.drop(signer_public, &k).expect("parcel known to be in self.current/self.future; qed");
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
    /// If there is already parcel with same `(sender, nonce)` it will be replaced iff `fee` is higher.
    /// One of the parcels is dropped from set and also removed from pool entirely (from `by_hash`).
    ///
    /// Returns `true` if parcel actually got to the pool (`false` if there was already a parcel with higher
    /// fee)
    fn replace_parcel(
        parcel: MemPoolItem,
        base_nonce: U256,
        set: &mut ParcelSet,
        by_hash: &mut HashMap<H256, MemPoolItem>,
        local: &mut LocalParcelsList,
    ) -> bool {
        let order = ParcelOrder::for_parcel(&parcel, base_nonce);
        let hash = parcel.hash();
        let signer_public = parcel.signer_public();
        let nonce = parcel.nonce();

        let old_hash = by_hash.insert(hash, parcel);
        assert!(old_hash.is_none(), "Each hash has to be inserted exactly once.");

        ctrace!(MEM_POOL, "Inserting: {:?}", order);

        if let Some(old) = set.insert(signer_public, nonce, order.clone()) {
            Self::replace_orders(signer_public, nonce, old, order, set, by_hash, local)
        } else {
            true
        }
    }

    fn replace_orders(
        signer_public: Public,
        nonce: U256,
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
            set.insert(signer_public, nonce, old);
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
    /// Number of future parcels (waiting for parcels with lower nonces first)
    pub future: usize,
}

#[derive(Debug)]
/// Details of account
pub struct AccountDetails {
    /// Most recent account nonce
    pub nonce: U256,
    /// Current account balance
    pub balance: U256,
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

fn check_if_removed(sender: &Public, nonce: &U256, dropped: Option<HashMap<Public, U256>>) -> Result<(), ParcelError> {
    match dropped {
        Some(dropped) => match dropped.get(sender) {
            Some(min) if nonce >= min => Err(ParcelError::LimitReached),
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

#[cfg(test)]
pub mod test {
    use std::cmp::Ordering;

    use ckey::{Generator, Random};
    use ctypes::parcel::{Parcel, ShardChange};
    use ctypes::transaction::{AssetMintOutput, Transaction};

    use super::*;

    #[test]
    fn ordering() {
        assert_eq!(ParcelOrigin::Local.cmp(&ParcelOrigin::External), Ordering::Less);
        assert_eq!(ParcelOrigin::RetractedBlock.cmp(&ParcelOrigin::Local), Ordering::Less);
        assert_eq!(ParcelOrigin::RetractedBlock.cmp(&ParcelOrigin::External), Ordering::Less);

        assert_eq!(ParcelOrigin::External.cmp(&ParcelOrigin::Local), Ordering::Greater);
        assert_eq!(ParcelOrigin::Local.cmp(&ParcelOrigin::RetractedBlock), Ordering::Greater);
        assert_eq!(ParcelOrigin::External.cmp(&ParcelOrigin::RetractedBlock), Ordering::Greater);
    }

    #[test]
    fn cost_of_empty_parcel_is_fee() {
        let fee = U256::from(100);
        let parcel = Parcel {
            nonce: U256::zero(),
            fee,
            network_id: "tc".into(),
            action: Action::AssetTransactionGroup {
                transactions: vec![],
                changes: vec![],
                signatures: vec![],
            },
        };
        let keypair = Random.generate().unwrap();
        let signed = SignedParcel::new_with_sign(parcel, keypair.private());
        let item = MemPoolItem::new(signed, ParcelOrigin::Local, 0, 0);

        assert_eq!(fee, item.cost());
    }

    #[test]
    fn mint_transaction_does_not_increase_cost() {
        let shard_id = 0xCCC;

        let fee = U256::from(100);
        let world_id = 0;
        let transactions = vec![Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            world_id,
            metadata: "Metadata".to_string(),
            output: AssetMintOutput {
                lock_script_hash: H256::zero(),
                parameters: vec![],
                amount: None,
            },
            registrar: None,
            nonce: 0,
        }];
        let parcel = Parcel {
            nonce: U256::zero(),
            fee,
            network_id: "tc".into(),
            action: Action::AssetTransactionGroup {
                transactions,
                changes: vec![ShardChange {
                    shard_id,
                    pre_root: H256::zero(),
                    post_root: H256::zero(),
                }],
                signatures: vec![],
            },
        };
        let keypair = Random.generate().unwrap();
        let signed = SignedParcel::new_with_sign(parcel, keypair.private());
        let item = MemPoolItem::new(signed, ParcelOrigin::Local, 0, 0);

        assert_eq!(fee, item.cost());
    }

    #[test]
    fn transfer_transaction_does_not_increase_cost() {
        let shard_id = 0;

        let fee = U256::from(100);
        let world_id = 0;
        let transactions = vec![
            Transaction::AssetMint {
                network_id: "tc".into(),
                shard_id,
                world_id,
                metadata: "Metadata".to_string(),
                output: AssetMintOutput {
                    lock_script_hash: H256::zero(),
                    parameters: vec![],
                    amount: None,
                },
                registrar: None,
                nonce: 0,
            },
            Transaction::AssetTransfer {
                network_id: "tc".into(),
                burns: vec![],
                inputs: vec![],
                outputs: vec![],
                nonce: 0,
            },
        ];
        let parcel = Parcel {
            nonce: U256::zero(),
            fee,
            network_id: "tc".into(),
            action: Action::AssetTransactionGroup {
                transactions,
                changes: vec![ShardChange {
                    shard_id,
                    pre_root: H256::zero(),
                    post_root: H256::zero(),
                }],
                signatures: vec![],
            },
        };
        let keypair = Random.generate().unwrap();
        let signed = SignedParcel::new_with_sign(parcel, keypair.private());
        let item = MemPoolItem::new(signed, ParcelOrigin::Local, 0, 0);

        assert_eq!(fee, item.cost());
    }

    #[test]
    fn payment_increases_cost() {
        let fee = U256::from(100);
        let amount = U256::from(100000);
        let receiver = 1u64.into();
        let keypair = Random.generate().unwrap();
        let parcel = Parcel {
            nonce: U256::zero(),
            fee,
            network_id: "tc".into(),
            action: Action::Payment {
                receiver,
                amount,
            },
        };
        let signed = SignedParcel::new_with_sign(parcel, keypair.private());
        let item = MemPoolItem::new(signed, ParcelOrigin::Local, 0, 0);

        assert_eq!(fee + amount, item.cost());
    }

    #[test]
    fn fee_per_byte_order_simple() {
        let order1 = create_parcel_order(U256::from(1000_000_000), 100);
        let order2 = create_parcel_order(U256::from(1500_000_000), 200);
        assert_eq!(true, order1.fee_per_byte > order2.fee_per_byte);
        assert_eq!(Ordering::Greater, order1.cmp(&order2));
    }

    #[test]
    fn fee_per_byte_order_sort() {
        let factors: Vec<Vec<usize>> = vec![
            vec![4, 9],   // 0.44
            vec![2, 9],   // 0.22
            vec![2, 6],   // 0.33
            vec![10, 10], // 1
            vec![2, 8],   // 0.25
        ];
        let mut orders: Vec<ParcelOrder> = Vec::new();
        for factor in factors {
            let fee: u64 = 1000_000 * (factor[0] as u64);
            orders.push(create_parcel_order(U256::from(fee), 10 * factor[1]));
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

    fn create_parcel_order(fee: U256, transaction_count: usize) -> ParcelOrder {
        let transaction = Transaction::AssetTransfer {
            network_id: "tc".into(),
            burns: vec![],
            inputs: vec![],
            outputs: vec![],
            nonce: 0,
        };
        let keypair = Random.generate().unwrap();
        let parcel = Parcel {
            nonce: U256::zero(),
            fee,
            network_id: "tc".into(),
            action: Action::AssetTransactionGroup {
                transactions: vec![transaction; transaction_count],
                changes: vec![ShardChange {
                    shard_id: 0,
                    pre_root: H256::zero(),
                    post_root: H256::zero(),
                }],
                signatures: vec![],
            },
        };
        let signed = SignedParcel::new_with_sign(parcel, keypair.private());
        let item = MemPoolItem::new(signed, ParcelOrigin::Local, 0, 0);
        ParcelOrder::for_parcel(&item, 0.into())
    }
}

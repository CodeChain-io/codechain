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

use super::super::parcel::{ParcelError, SignedParcel};
use super::super::types::BlockNumber;
use super::super::Transaction;
use super::local_parcels::{LocalParcelsList, Status as LocalParcelStatus};
use super::ParcelImportResult;

/// Parcel with the same (sender, nonce) can be replaced only if
/// `new_fee > old_fee + old_fee >> SHIFT`
const FEE_BUMP_SHIFT: usize = 3; // 2 = 25%, 3 = 12.5%, 4 = 6.25%

/// Point in time when parcel was inserted.
pub type QueuingInstant = BlockNumber;
const DEFAULT_QUEUING_PERIOD: BlockNumber = 128;

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
    /// Heap usage of this parcel.
    mem_usage: usize,
    /// Hash to identify associated parcel
    hash: H256,
    /// Incremental id assigned when parcel is inserted to the queue.
    insertion_id: u64,
    /// Origin of the parcel
    origin: ParcelOrigin,
}

impl ParcelOrder {
    fn for_parcel(parcel: &QueuedParcel, base_nonce: U256) -> Self {
        Self {
            nonce_height: parcel.nonce() - base_nonce,
            fee: parcel.parcel.fee,
            mem_usage: parcel.parcel.heap_size_of_children(),
            hash: parcel.hash(),
            insertion_id: parcel.insertion_id,
            origin: parcel.origin,
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

        // Then compare fee
        if self.fee != b.fee {
            return b.fee.cmp(&self.fee)
        }

        // Lastly compare insertion_id
        self.insertion_id.cmp(&b.insertion_id)
    }
}

/// Queued Parcel
#[derive(Debug)]
struct QueuedParcel {
    /// Parcel.
    parcel: SignedParcel,
    /// Parcel origin.
    origin: ParcelOrigin,
    /// Insertion time
    insertion_time: QueuingInstant,
    /// ID assigned upon insertion, should be unique.
    insertion_id: u64,
}

impl QueuedParcel {
    fn new(parcel: SignedParcel, origin: ParcelOrigin, insertion_time: QueuingInstant, insertion_id: u64) -> Self {
        QueuedParcel {
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

    fn sender(&self) -> Address {
        self.parcel.sender()
    }

    fn cost(&self) -> U256 {
        let value = match (*self.parcel).transaction {
            Transaction::Payment {
                value,
                ..
            } => value,
            _ => U256::from(0),
        };
        value + self.parcel.fee
    }
}

/// Holds parcels accessible by (address, nonce) and by priority
struct ParcelSet {
    by_priority: BTreeSet<ParcelOrder>,
    by_address: Table<Address, U256, ParcelOrder>,
    by_fee: MultiMap<U256, H256>,
    limit: usize,
    memory_limit: usize,
}

impl ParcelSet {
    /// Inserts `ParcelOrder` to this set. Parcel does not need to be unique -
    /// the same parcel may be validly inserted twice. Any previous parcel that
    /// it replaces (i.e. with the same `sender` and `nonce`) should be returned.
    fn insert(&mut self, sender: Address, nonce: U256, order: ParcelOrder) -> Option<ParcelOrder> {
        if !self.by_priority.insert(order.clone()) {
            return Some(order.clone())
        }
        let order_hash = order.hash.clone();
        let order_fee = order.fee.clone();
        let by_address_replaced = self.by_address.insert(sender, nonce, order);
        if let Some(ref old_order) = by_address_replaced {
            assert!(
                self.by_priority.remove(old_order),
                "hash is in `by_address`; all parcels in `by_address` must be in `by_priority`; qed"
            );
            assert!(
                self.by_fee.remove(&old_order.fee, &old_order.hash),
                "hash is in `by_address`; all parcels' fee in `by_address` must be in `by_fee`; qed"
            );
        }
        self.by_fee.insert(order_fee, order_hash);
        assert_eq!(self.by_priority.len(), self.by_address.len());
        assert_eq!(self.by_fee.values().map(|v| v.len()).fold(0, |a, b| a + b), self.by_address.len());
        by_address_replaced
    }

    /// Remove low priority parcels if there is more than specified by given `limit`.
    ///
    /// It drops parecls from this set but also removes associated `VerifiedParcel`.
    /// Returns addresses and lowest nonces of parcels removed because of limit.
    fn enforce_limit(
        &mut self,
        by_hash: &mut HashMap<H256, QueuedParcel>,
        local: &mut LocalParcelsList,
    ) -> Option<HashMap<Address, U256>> {
        let mut count = 0;
        let mut mem_usage = 0;
        let to_drop: Vec<(Address, U256)> = {
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
                        "All parcels in `self.by_priority` and `self.by_address` are kept in sync with `by_hash`.",
                    )
                })
                .map(|parcel| (parcel.sender(), parcel.nonce()))
                .collect()
        };

        Some(to_drop.into_iter().fold(HashMap::new(), |mut removed, (sender, nonce)| {
            let order = self.drop(&sender, &nonce)
                .expect("Parcel has just been found in `by_priority`; so it is in `by_address` also.");
            trace!(target: "parcelqueue", "Dropped out of limit parcel: {:?}", order.hash);

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

    /// Drop parcel from this set (remove from `by_priority` and `by_address`)
    fn drop(&mut self, sender: &Address, nonce: &U256) -> Option<ParcelOrder> {
        if let Some(parcel_order) = self.by_address.remove(sender, nonce) {
            assert!(
                self.by_fee.remove(&parcel_order.fee, &parcel_order.hash),
                "hash is in `by_address`; all parcels' fee in `by_address` must be in `by_fee`; qed"
            );
            assert!(
                self.by_priority.remove(&parcel_order),
                "hash is in `by_address`; all parcels in `by_address` must be in `by_priority`; qed"
            );
            assert_eq!(self.by_priority.len(), self.by_address.len());
            assert_eq!(self.by_fee.values().map(|v| v.len()).fold(0, |a, b| a + b), self.by_address.len());
            return Some(parcel_order)
        }
        assert_eq!(self.by_priority.len(), self.by_address.len());
        assert_eq!(self.by_fee.values().map(|v| v.len()).fold(0, |a, b| a + b), self.by_address.len());
        None
    }

    /// Drop all parcels.
    fn clear(&mut self) {
        self.by_priority.clear();
        self.by_address.clear();
    }

    /// Sets new limit for number of parcels in this `ParcelSet`.
    /// Note the limit is not applied (no parcels are removed) by calling this method.
    fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }

    /// Get the minimum fee that we can accept into this queue that wouldn't cause the parcel to
    /// immediately be dropped. 0 if the queue isn't at capacity; 1 plus the lowest if it is.
    fn fee_entry_limit(&self) -> U256 {
        match self.by_fee.keys().next() {
            Some(k) if self.by_priority.len() >= self.limit => *k + 1.into(),
            _ => U256::default(),
        }
    }
}

pub struct ParcelQueue {
    /// Fee threshold for parcels that can be imported to this queue (defaults to 0)
    minimal_fee: U256,
    /// Maximal time parcel may occupy the queue.
    /// When we reach `max_time_in_queue / 2^3` we re-validate
    /// account balance.
    max_time_in_queue: QueuingInstant,
    /// Priority queue for parcels that can go to block
    current: ParcelSet,
    /// Priority queue for parcels that has been received but are not yet valid to go to block
    future: ParcelSet,
    /// All parcels managed by queue indexed by hash
    by_hash: HashMap<H256, QueuedParcel>,
    /// Last nonce of parcel in current (to quickly check next expected parcel)
    last_nonces: HashMap<Address, U256>,
    /// List of local parcels and their statuses.
    local_parcels: LocalParcelsList,
    /// Next id that should be assigned to a parcel imported to the queue.
    next_parcel_id: u64,
}

impl Default for ParcelQueue {
    fn default() -> Self {
        ParcelQueue::new()
    }
}

impl ParcelQueue {
    /// Creates new instance of this Queue
    pub fn new() -> Self {
        Self::with_limits(8192, usize::max_value())
    }

    /// Create new instance of this Queue with specified limits
    pub fn with_limits(limit: usize, memory_limit: usize) -> Self {
        let current = ParcelSet {
            by_priority: BTreeSet::new(),
            by_address: Table::new(),
            by_fee: MultiMap::default(),
            limit,
            memory_limit,
        };

        let future = ParcelSet {
            by_priority: BTreeSet::new(),
            by_address: Table::new(),
            by_fee: MultiMap::default(),
            limit,
            memory_limit,
        };

        ParcelQueue {
            minimal_fee: U256::zero(),
            max_time_in_queue: DEFAULT_QUEUING_PERIOD,
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

    /// Returns current limit of parcels in the queue.
    pub fn limit(&self) -> usize {
        self.current.limit
    }

    /// Get the minimal fee.
    pub fn minimal_fee(&self) -> &U256 {
        &self.minimal_fee
    }

    /// Sets new fee threshold for incoming parcels.
    /// Any parcel already imported to the queue is not affected.
    pub fn set_minimal_fee(&mut self, min_fee: U256) {
        self.minimal_fee = min_fee;
    }

    /// Get one more than the lowest fee in the queue iff the pool is
    /// full, otherwise 0.
    pub fn effective_minimum_fee(&self) -> U256 {
        self.current.fee_entry_limit()
    }

    /// Returns current status for this queue
    pub fn status(&self) -> ParcelQueueStatus {
        ParcelQueueStatus {
            pending: self.current.by_priority.len(),
            future: self.future.by_priority.len(),
        }
    }

    /// Add signed parcel to queue to be verified and imported.
    ///
    /// NOTE details_provider methods should be cheap to compute
    /// otherwise it might open up an attack vector.
    pub fn add(
        &mut self,
        parcel: SignedParcel,
        origin: ParcelOrigin,
        time: QueuingInstant,
        details_provider: &ParcelDetailsProvider,
    ) -> Result<ParcelImportResult, ParcelError> {
        if origin == ParcelOrigin::Local {
            let hash = parcel.hash();
            let closed_parcel = parcel.clone();

            let result = self.add_internal(parcel, origin, time, details_provider);
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
            self.add_internal(parcel, origin, time, details_provider)
        }
    }

    /// Checks the current nonce for all parcels' senders in the queue and removes the old parcels.
    pub fn remove_old<F>(&mut self, fetch_account: &F, current_time: QueuingInstant)
    where
        F: Fn(&Address) -> AccountDetails, {
        let senders = self.current
            .by_address
            .keys()
            .chain(self.future.by_address.keys())
            .map(|sender| (*sender, fetch_account(sender)))
            .collect::<HashMap<_, _>>();

        for (sender, details) in senders.iter() {
            self.cull(*sender, details.nonce);
        }

        let max_time = self.max_time_in_queue;
        let balance_check = max_time >> 3;
        // Clear parcels occupying the queue too long
        let invalid = self.by_hash
            .iter()
            .filter(|&(_, ref parcel)| !parcel.origin.is_local())
            .map(|(hash, parcel)| (hash, parcel, current_time.saturating_sub(parcel.insertion_time)))
            .filter_map(|(hash, parcel, time_diff)| {
                if time_diff > max_time {
                    return Some(*hash)
                }

                if time_diff > balance_check {
                    return match senders.get(&parcel.sender()) {
                        Some(details) if parcel.cost() > details.balance => Some(*hash),
                        _ => None,
                    }
                }

                None
            })
            .collect::<Vec<_>>();
        let fetch_nonce =
            |a: &Address| senders.get(a).expect("We fetch details for all senders from both current and future").nonce;
        for hash in invalid {
            self.remove(&hash, &fetch_nonce, RemovalReason::Invalid);
        }
    }

    /// Removes invalid parcel identified by hash from queue.
    /// Assumption is that this parcel nonce is not related to client nonce,
    /// so parcels left in queue are processed according to client nonce.
    ///
    /// If gap is introduced marks subsequent parcels as future
    pub fn remove<F>(&mut self, parcel_hash: &H256, fetch_nonce: &F, reason: RemovalReason)
    where
        F: Fn(&Address) -> U256, {
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
        let parcel = self.by_hash.remove(parcel_hash);
        if parcel.is_none() {
            // We don't know this parcel
            return
        }

        let parcel = parcel.expect("None is tested in early-exit condition above; qed");
        let sender = parcel.sender();
        let nonce = parcel.nonce();
        let current_nonce = fetch_nonce(&sender);

        trace!(target: "parcelqueue", "Removing invalid parcel: {:?}", parcel.hash());

        // Mark in locals
        if self.local_parcels.contains(parcel_hash) {
            match reason {
                RemovalReason::Invalid => self.local_parcels.mark_invalid(parcel.parcel.into()),
                RemovalReason::NotAllowed => self.local_parcels.mark_invalid(parcel.parcel.into()),
                RemovalReason::Canceled => self.local_parcels.mark_canceled(parcel.parcel.into()),
            }
        }

        // Remove from future
        let order = self.future.drop(&sender, &nonce);
        if order.is_some() {
            self.update_future(&sender, current_nonce);
            // And now lets check if there is some chain of parcels in future
            // that should be placed in current
            self.move_matching_future_to_current(sender, current_nonce, current_nonce);
            assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
            return
        }

        // Remove from current
        let order = self.current.drop(&sender, &nonce);
        if order.is_some() {
            // This will keep consistency in queue
            // Moves all to future and then promotes a batch from current:
            self.cull_internal(sender, current_nonce);
            assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
            return
        }
    }

    /// Removes all parcels from particular sender up to (excluding) given client (state) nonce.
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

    /// Finds parcel in the queue by hash (if any)
    pub fn find(&self, hash: &H256) -> Option<SignedParcel> {
        self.by_hash.get(hash).map(|parcel| parcel.parcel.clone())
    }

    /// Returns highest parcel nonce for given address.
    pub fn last_nonce(&self, address: &Address) -> Option<U256> {
        self.last_nonces.get(address).cloned()
    }

    /// Returns top parcels from the queue ordered by priority.
    pub fn top_parcels(&self) -> Vec<SignedParcel> {
        self.current
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

    /// Returns local parcels (some of them might not be part of the queue anymore).
    pub fn local_parcels(&self) -> &LinkedHashMap<H256, LocalParcelStatus> {
        self.local_parcels.all_parcels()
    }

    /// Adds signed parcel to the queue.
    fn add_internal(
        &mut self,
        parcel: SignedParcel,
        origin: ParcelOrigin,
        time: QueuingInstant,
        details_provider: &ParcelDetailsProvider,
    ) -> Result<ParcelImportResult, ParcelError> {
        if origin != ParcelOrigin::Local && parcel.fee < self.minimal_fee {
            trace!(target: "parcelqueue",
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

        let full_queues_lowest = self.effective_minimum_fee();
        if parcel.fee < full_queues_lowest && origin != ParcelOrigin::Local {
            trace!(target: "parcelqueue",
                   "Dropping parcel below lowest fee in a full queue: {:?} (gp: {} < {})",
                   parcel.hash(),
                   parcel.fee,
                   full_queues_lowest
            );

            return Err(ParcelError::InsufficientFee {
                minimal: full_queues_lowest,
                got: parcel.fee,
            })
        }

        let client_account = details_provider.fetch_account(&parcel.sender());
        if client_account.balance < parcel.fee {
            trace!(target: "parcelqueue",
                   "Dropping parcel without sufficient balance: {:?} ({} < {})",
                   parcel.hash(),
                   client_account.balance,
                   parcel.fee
            );

            return Err(ParcelError::InsufficientBalance {
                cost: parcel.fee,
                balance: client_account.balance,
            })
        }
        parcel.check_low_s()?;
        // No invalid parcels beyond this point.
        let id = self.next_parcel_id;
        self.next_parcel_id += 1;
        let vparcel = QueuedParcel::new(parcel, origin, time, id);
        let r = self.import_parcel(vparcel, client_account.nonce);
        assert_eq!(self.future.by_priority.len() + self.current.by_priority.len(), self.by_hash.len());
        r
    }

    /// Adds VerifiedParcel to this queue.
    ///
    /// Determines if it should be placed in current or future. When parcel is
    /// imported to `current` also checks if there are any `future` parcels that should be promoted because of
    /// this.
    ///
    /// It ignores parcels that has already been imported (same `hash`) and replaces the parcel
    /// iff `(address, nonce)` is the same but `fee` is higher.
    ///
    /// Returns `true` when parcel was imported successfully
    fn import_parcel(&mut self, parcel: QueuedParcel, state_nonce: U256) -> Result<ParcelImportResult, ParcelError> {
        if self.by_hash.get(&parcel.hash()).is_some() {
            // Parcel is already imported.
            trace!(target: "parcelqueue", "Dropping already imported parcel: {:?}", parcel.hash());
            return Err(ParcelError::AlreadyImported)
        }

        let address = parcel.sender();
        let nonce = parcel.nonce();
        let hash = parcel.hash();

        // The parcel might be old, let's check that.
        // This has to be the first test, otherwise calculating
        // nonce height would result in overflow.
        if nonce < state_nonce {
            // Droping parcel
            trace!(target: "parcelqueue", "Dropping old parcel: {:?} (nonce: {} < {})", parcel.hash(), nonce, state_nonce);
            return Err(ParcelError::Old)
        }

        // Update nonces of parcels in future (remove old parcels)
        self.update_future(&address, state_nonce);
        // State nonce could be updated. Maybe there are some more items waiting in future?
        self.move_matching_future_to_current(address, state_nonce, state_nonce);
        // Check the next expected nonce (might be updated by move above)
        let next_nonce = self.last_nonces.get(&address).cloned().map_or(state_nonce, |n| n + U256::one());

        if parcel.origin.is_local() {
            self.mark_parcels_local(&address);
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
            check_if_removed(&address, &nonce, removed)?;

            debug!(target: "parcelqueue", "Importing parcel to future: {:?}", hash);
            debug!(target: "parcelqueue", "status: {:?}", self.status());
            return Ok(ParcelImportResult::Future)
        }

        // We might have filled a gap - move some more parcels from future
        self.move_matching_future_to_current(address, nonce, state_nonce);
        self.move_matching_future_to_current(address, nonce + U256::one(), state_nonce);

        // Replace parcel if any
        check_too_cheap(Self::replace_parcel(
            parcel,
            state_nonce,
            &mut self.current,
            &mut self.by_hash,
            &mut self.local_parcels,
        ))?;
        // Keep track of highest nonce stored in current
        let new_max = self.last_nonces.get(&address).map_or(nonce, |n| cmp::max(nonce, *n));
        self.last_nonces.insert(address, new_max);

        // Also enforce the limit
        let removed = self.current.enforce_limit(&mut self.by_hash, &mut self.local_parcels);
        // If some parcel were removed because of limit we need to update last_nonces also.
        self.update_last_nonces(&removed);
        // Trigger error if the parcel we are importing was removed.
        check_if_removed(&address, &nonce, removed)?;

        debug!(target: "parcelqueue", "Imported parcel to current: {:?}", hash);
        debug!(target: "parcelqueue", "status: {:?}", self.status());
        Ok(ParcelImportResult::Current)
    }

    /// Always updates future and moves parcel from current to future.
    fn cull_internal(&mut self, sender: Address, client_nonce: U256) {
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

    /// Update height of all parcels in future parcels set.
    fn update_future(&mut self, sender: &Address, current_nonce: U256) {
        // We need to drain all parcels for current sender from future and reinsert them with updated height
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
                trace!(target: "parcelqueue", "Removing old parcel: {:?} (nonce: {} < {})", order.hash, k, current_nonce);
                // Remove the parcel completely
                self.by_hash.remove(&order.hash).expect("All parcels in `future` are also in `by_hash`");
            }
        }
    }

    /// Checks if there are any parcels in `future` that should actually be promoted to `current`
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
                    self.local_parcels.mark_pending(order.hash);
                }
                if let Some(old) = self.current.insert(address, current_nonce, order.clone()) {
                    Self::replace_orders(
                        address,
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
        self.future.by_address.clear_if_empty(&address);
        if let Some(x) = update_last_nonce_to {
            // Update last inserted nonce
            self.last_nonces.insert(address, x);
        }
    }

    /// Drop all parcels from given sender from `current`.
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
                    self.local_parcels.mark_future(order.hash);
                }
                if let Some(old) = self.future.insert(*sender, k, order.clone()) {
                    Self::replace_orders(
                        *sender,
                        k,
                        old,
                        order,
                        &mut self.future,
                        &mut self.by_hash,
                        &mut self.local_parcels,
                    );
                }
            } else {
                trace!(target: "parcelqueue", "Removing old parcel: {:?} (nonce: {} < {})", order.hash, k, current_nonce);
                let parcel = self.by_hash.remove(&order.hash).expect("All parcels in `future` are also in `by_hash`");
                if parcel.origin.is_local() {
                    self.local_parcels.mark_mined(parcel.parcel);
                }
            }
        }
        self.future.enforce_limit(&mut self.by_hash, &mut self.local_parcels);
    }

    /// Marks all parcels from particular sender as local parcels
    fn mark_parcels_local(&mut self, sender: &Address) {
        fn mark_local<F: FnMut(H256)>(sender: &Address, set: &mut ParcelSet, mut mark: F) {
            // Mark all parcels from this sender as local
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
                let mut order = set.drop(sender, &k).expect("parcel known to be in self.current/self.future; qed");
                order.origin = ParcelOrigin::Local;
                mark(order.hash);
                set.insert(*sender, k, order);
            }
        }

        let local = &mut self.local_parcels;
        mark_local(sender, &mut self.current, |hash| local.mark_pending(hash));
        mark_local(sender, &mut self.future, |hash| local.mark_future(hash));
    }

    /// Replaces parcel in given set (could be `future` or `current`).
    ///
    /// If there is already parcel with same `(sender, nonce)` it will be replaced iff `fee` is higher.
    /// One of the parcels is dropped from set and also removed from queue entirely (from `by_hash`).
    ///
    /// Returns `true` if parcel actually got to the queue (`false` if there was already a parcel with higher
    /// fee)
    fn replace_parcel(
        parcel: QueuedParcel,
        base_nonce: U256,
        set: &mut ParcelSet,
        by_hash: &mut HashMap<H256, QueuedParcel>,
        local: &mut LocalParcelsList,
    ) -> bool {
        let order = ParcelOrder::for_parcel(&parcel, base_nonce);
        let hash = parcel.hash();
        let address = parcel.sender();
        let nonce = parcel.nonce();

        let old_hash = by_hash.insert(hash, parcel);
        assert!(old_hash.is_none(), "Each hash has to be inserted exactly once.");

        trace!(target: "parcelqueue", "Inserting: {:?}", order);

        if let Some(old) = set.insert(address, nonce, order.clone()) {
            Self::replace_orders(address, nonce, old, order, set, by_hash, local)
        } else {
            true
        }
    }

    fn replace_orders(
        address: Address,
        nonce: U256,
        old: ParcelOrder,
        order: ParcelOrder,
        set: &mut ParcelSet,
        by_hash: &mut HashMap<H256, QueuedParcel>,
        local: &mut LocalParcelsList,
    ) -> bool {
        // There was already parcel in queue. Let's check which one should stay
        let old_hash = old.hash;
        let new_hash = order.hash;

        let old_fee = old.fee;
        let new_fee = order.fee;
        let min_required_fee = old_fee + (old_fee >> FEE_BUMP_SHIFT);

        if min_required_fee > new_fee {
            trace!(target: "parcelqueue", "Didn't insert parcel because fee was too low: {:?} ({:?} stays in the queue)", order.hash, old.hash);
            // Put back old parcel since it has greater priority (higher fee)
            set.insert(address, nonce, old);
            // and remove new one
            let order = by_hash
                .remove(&order.hash)
                .expect("The hash has been just inserted and no other line is altering `by_hash`.");
            if order.origin.is_local() {
                local.mark_replaced(order.parcel, old_fee, old_hash);
            }
            false
        } else {
            trace!(target: "parcelqueue", "Replaced parcel: {:?} with parcel with higher fee: {:?}", old.hash, order.hash);
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
/// Current status of the queue
pub struct ParcelQueueStatus {
    /// Number of pending parcels (ready to go to block)
    pub pending: usize,
    /// Number of future parcels (waiting for parcels with lower nonces first)
    pub future: usize,
}

/// `ParcelQueue` parcel details provider.
pub trait ParcelDetailsProvider {
    /// Fetch parcel-related account details.
    fn fetch_account(&self, address: &Address) -> AccountDetails;
}

#[derive(Debug)]
/// Details of account
pub struct AccountDetails {
    /// Most recent account nonce
    pub nonce: U256,
    /// Current account balance
    pub balance: U256,
}

/// Reason to remove single parcel from the queue.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RemovalReason {
    /// Parcel is invalid
    Invalid,
    /// Parcel was canceled
    Canceled,
    /// Parcel is not allowed,
    NotAllowed,
}

fn check_too_cheap(is_in: bool) -> Result<(), ParcelError> {
    if is_in {
        Ok(())
    } else {
        Err(ParcelError::TooCheapToReplace)
    }
}

fn check_if_removed(
    sender: &Address,
    nonce: &U256,
    dropped: Option<HashMap<Address, U256>>,
) -> Result<(), ParcelError> {
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
    use super::ParcelOrigin;
    use std::cmp::Ordering;

    #[test]
    fn test_ordering() {
        assert_eq!(ParcelOrigin::Local.cmp(&ParcelOrigin::External), Ordering::Less);
        assert_eq!(ParcelOrigin::RetractedBlock.cmp(&ParcelOrigin::Local), Ordering::Less);
        assert_eq!(ParcelOrigin::RetractedBlock.cmp(&ParcelOrigin::External), Ordering::Less);

        assert_eq!(ParcelOrigin::External.cmp(&ParcelOrigin::Local), Ordering::Greater);
        assert_eq!(ParcelOrigin::Local.cmp(&ParcelOrigin::RetractedBlock), Ordering::Greater);
        assert_eq!(ParcelOrigin::External.cmp(&ParcelOrigin::RetractedBlock), Ordering::Greater);
    }
}

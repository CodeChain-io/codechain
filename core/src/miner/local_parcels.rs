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

use ctypes::{H256, U256};
use linked_hash_map::LinkedHashMap;

use super::super::parcel::{ParcelError, SignedParcel};

/// Status of local parcel.
/// Can indicate that the parcel is currently part of the queue (`Pending/Future`)
/// or gives a reason why the parcel was removed.
#[derive(Debug, PartialEq, Clone)]
pub enum Status {
    /// The parcel is currently in the mem pool.
    Pending,
    /// The parcel is in future part of the mem pool.
    Future,
    /// Parcel is already mined.
    Mined(SignedParcel),
    /// Parcel is dropped because of limit
    Dropped(SignedParcel),
    /// Replaced because of higher gas price of another parcel.
    Replaced(SignedParcel, U256, H256),
    /// Parcel was never accepted to the mem pool.
    Rejected(SignedParcel, ParcelError),
    /// Parcel is invalid.
    Invalid(SignedParcel),
    /// Parcel was canceled.
    Canceled(SignedParcel),
}

impl Status {
    fn is_current(&self) -> bool {
        *self == Status::Pending || *self == Status::Future
    }
}

/// Keeps track of local parcels that are in the queue or were mined/dropped recently.
#[derive(Debug)]
pub struct LocalParcelsList {
    max_old: usize,
    parcels: LinkedHashMap<H256, Status>,
}

impl Default for LocalParcelsList {
    fn default() -> Self {
        Self::new(10)
    }
}

impl LocalParcelsList {
    /// Create a new list of local parcels.
    pub fn new(max_old: usize) -> Self {
        LocalParcelsList {
            max_old,
            parcels: Default::default(),
        }
    }

    /// Mark parcel with given hash as pending.
    pub fn mark_pending(&mut self, hash: H256) {
        cdebug!(OWN_PARCEL, "Imported to Current (hash {:?})", hash);
        self.clear_old();
        self.parcels.insert(hash, Status::Pending);
    }

    /// Mark parcel with given hash as future.
    pub fn mark_future(&mut self, hash: H256) {
        cdebug!(OWN_PARCEL, "Imported to Future (hash {:?})", hash);
        self.parcels.insert(hash, Status::Future);
        self.clear_old();
    }

    /// Mark given parcel as rejected from the queue.
    pub fn mark_rejected(&mut self, parcel: SignedParcel, err: ParcelError) {
        cdebug!(OWN_PARCEL, "Parcel rejected (hash {:?}): {:?}", parcel.hash(), err);
        self.parcels.insert(parcel.hash(), Status::Rejected(parcel, err));
        self.clear_old();
    }

    /// Mark the parcel as replaced by parcel with given hash.
    pub fn mark_replaced(&mut self, parcel: SignedParcel, gas_price: U256, hash: H256) {
        cdebug!(
            OWN_PARCEL,
            "Parcel replaced (hash {:?}) by {:?} (new gas price: {:?})",
            parcel.hash(),
            hash,
            gas_price
        );
        self.parcels.insert(parcel.hash(), Status::Replaced(parcel, gas_price, hash));
        self.clear_old();
    }

    /// Mark parcel as invalid.
    pub fn mark_invalid(&mut self, signed: SignedParcel) {
        cwarn!(OWN_PARCEL, "Parcel marked invalid (hash {:?})", signed.hash());
        self.parcels.insert(signed.hash(), Status::Invalid(signed));
        self.clear_old();
    }

    /// Mark parcel as canceled.
    pub fn mark_canceled(&mut self, signed: SignedParcel) {
        cwarn!(OWN_PARCEL, "Parcel canceled (hash {:?})", signed.hash());
        self.parcels.insert(signed.hash(), Status::Canceled(signed));
        self.clear_old();
    }

    /// Mark parcel as dropped because of limit.
    pub fn mark_dropped(&mut self, signed: SignedParcel) {
        cwarn!(OWN_PARCEL, "Parcel dropped (hash {:?})", signed.hash());
        self.parcels.insert(signed.hash(), Status::Dropped(signed));
        self.clear_old();
    }

    /// Mark parcel as mined.
    pub fn mark_mined(&mut self, signed: SignedParcel) {
        cinfo!(OWN_PARCEL, "Parcel mined (hash {:?})", signed.hash());
        self.parcels.insert(signed.hash(), Status::Mined(signed));
        self.clear_old();
    }

    /// Returns true if the parcel is already in local parcels.
    pub fn contains(&self, hash: &H256) -> bool {
        self.parcels.contains_key(hash)
    }

    /// Return a map of all currently stored parcels.
    pub fn all_parcels(&self) -> &LinkedHashMap<H256, Status> {
        &self.parcels
    }

    fn clear_old(&mut self) {
        let number_of_old = self.parcels.values().filter(|status| !status.is_current()).count();

        if self.max_old >= number_of_old {
            return
        }

        let to_remove = self.parcels
            .iter()
            .filter(|&(_, status)| !status.is_current())
            .map(|(hash, _)| *hash)
            .take(number_of_old - self.max_old)
            .collect::<Vec<_>>();

        for hash in to_remove {
            self.parcels.remove(&hash);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::parcel;
    use super::*;
    use ckey::{Generator, Random};
    use ctypes::U256;

    #[test]
    fn should_add_parcel_as_pending() {
        // given
        let mut list = LocalParcelsList::default();

        // when
        list.mark_pending(10.into());
        list.mark_future(20.into());

        // then
        assert!(list.contains(&10.into()), "Should contain the parcel.");
        assert!(list.contains(&20.into()), "Should contain the parcel.");
        let statuses = list.all_parcels().values().cloned().collect::<Vec<Status>>();
        assert_eq!(statuses, vec![Status::Pending, Status::Future]);
    }

    #[test]
    fn should_clear_old_parcels() {
        // given
        let mut list = LocalParcelsList::new(1);
        let parcel1 = new_parcel(10.into());
        let parcel1_hash = parcel1.hash();
        let parcel2 = new_parcel(50.into());
        let parcel2_hash = parcel2.hash();

        list.mark_pending(10.into());
        list.mark_invalid(parcel1);
        list.mark_dropped(parcel2);
        assert!(list.contains(&parcel2_hash));
        assert!(!list.contains(&parcel1_hash));
        assert!(list.contains(&10.into()));

        // when
        list.mark_future(15.into());

        // then
        assert!(list.contains(&10.into()));
        assert!(list.contains(&15.into()));
    }

    fn new_parcel(nonce: U256) -> SignedParcel {
        let keypair = Random.generate().unwrap();
        let transactions = vec![];
        parcel::Parcel {
            nonce,
            fee: U256::from(1245),
            action: parcel::Action::ChangeShardState {
                transactions,
            },
            network_id: 0u64,
        }.sign(keypair.private())
    }
}

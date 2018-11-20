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

use ccrypto::blake256;
use ctypes::BlockNumber;
use primitives::H256;
use rlp::Rlp;

use super::ParcelView;
use crate::parcel::{LocalizedParcel, UnverifiedParcel};

/// View onto block rlp.
pub struct BodyView<'a> {
    rlp: Rlp<'a>,
}

impl<'a> BodyView<'a> {
    /// Creates new view onto block from raw bytes.
    pub fn new(bytes: &'a [u8]) -> BodyView<'a> {
        BodyView {
            rlp: Rlp::new(bytes),
        }
    }

    /// Creates new view onto block from rlp.
    pub fn new_from_rlp(rlp: Rlp<'a>) -> BodyView<'a> {
        BodyView {
            rlp,
        }
    }

    /// Return reference to underlaying rlp.
    pub fn rlp(&self) -> &Rlp<'a> {
        &self.rlp
    }

    /// Return List of parcels in given block.
    pub fn parcels(&self) -> Vec<UnverifiedParcel> {
        self.rlp.list_at(0)
    }

    /// Return List of parcels with additional localization info.
    pub fn localized_parcels(&self, block_hash: &H256, block_number: BlockNumber) -> Vec<LocalizedParcel> {
        self.parcels()
            .into_iter()
            .enumerate()
            .map(|(parcel_index, signed)| LocalizedParcel {
                signed,
                block_hash: block_hash.clone(),
                block_number,
                parcel_index,
                cached_signer_public: None,
            })
            .collect()
    }

    /// Return number of parcels in given block, without deserializing them.
    pub fn parcels_count(&self) -> usize {
        self.rlp.at(0).item_count()
    }

    /// Return List of parcels in given block.
    pub fn parcel_views(&self) -> Vec<ParcelView<'a>> {
        self.rlp.at(0).iter().map(ParcelView::new_from_rlp).collect()
    }

    /// Return parcel hashes.
    pub fn parcel_hashes(&self) -> Vec<H256> {
        self.rlp.at(0).iter().map(|rlp| blake256(rlp.as_raw())).collect()
    }

    /// Returns parcel at given index without deserializing unnecessary data.
    pub fn parcel_at(&self, index: usize) -> Option<UnverifiedParcel> {
        self.rlp.at(0).iter().nth(index).map(|rlp| rlp.as_val())
    }

    /// Returns localized parcel at given index.
    pub fn localized_parcel_at(
        &self,
        block_hash: &H256,
        block_number: BlockNumber,
        parcel_index: usize,
    ) -> Option<LocalizedParcel> {
        self.parcel_at(parcel_index).map(|signed| LocalizedParcel {
            signed,
            block_hash: block_hash.clone(),
            block_number,
            parcel_index,
            cached_signer_public: None,
        })
    }
}

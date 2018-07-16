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
use ctypes::Address;
use primitives::{Bytes, H256, U256};
use rlp::{self, Rlp};

use super::super::types::BlockNumber;

/// View onto block header rlp.
pub struct HeaderView<'a> {
    rlp: Rlp<'a>,
}

impl<'a> HeaderView<'a> {
    /// Creates new view onto header from raw bytes.
    pub fn new(bytes: &'a [u8]) -> HeaderView<'a> {
        HeaderView {
            rlp: Rlp::new(bytes),
        }
    }

    /// Creates new view onto header from rlp.
    pub fn new_from_rlp(rlp: Rlp<'a>) -> HeaderView<'a> {
        HeaderView {
            rlp,
        }
    }

    /// Returns header hash.
    pub fn hash(&self) -> H256 {
        blake256(self.rlp.as_raw())
    }

    /// Returns raw rlp.
    pub fn rlp(&self) -> &Rlp<'a> {
        &self.rlp
    }

    /// Returns parent hash.
    pub fn parent_hash(&self) -> H256 {
        self.rlp.val_at(0)
    }

    /// Returns author.
    pub fn author(&self) -> Address {
        self.rlp.val_at(1)
    }

    /// Returns state root.
    pub fn state_root(&self) -> H256 {
        self.rlp.val_at(2)
    }

    /// Returns parcels root.
    pub fn parcels_root(&self) -> H256 {
        self.rlp.val_at(3)
    }

    /// Returns block invoices root.
    pub fn invoices_root(&self) -> H256 {
        self.rlp.val_at(4)
    }

    /// Returns block score.
    pub fn score(&self) -> U256 {
        self.rlp.val_at(5)
    }

    /// Returns block number.
    pub fn number(&self) -> BlockNumber {
        self.rlp.val_at(6)
    }

    /// Returns timestamp.
    pub fn timestamp(&self) -> u64 {
        self.rlp.val_at(7)
    }

    /// Returns block extra data.
    pub fn extra_data(&self) -> Bytes {
        self.rlp.val_at(8)
    }

    /// Returns a vector of post-RLP-encoded seal fields.
    pub fn seal(&self) -> Vec<Bytes> {
        let mut seal = vec![];
        for i in 9..self.rlp.item_count() {
            seal.push(self.rlp.at(i).as_raw().to_vec());
        }
        seal
    }

    /// Returns a vector of seal fields (RLP-decoded).
    pub fn decode_seal(&self) -> Result<Vec<Bytes>, rlp::DecoderError> {
        let seal = self.seal();
        seal.into_iter().map(|s| rlp::UntrustedRlp::new(&s).data().map(|x| x.to_vec())).collect()
    }
}

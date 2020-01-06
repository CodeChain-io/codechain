// Copyright 2018-2019 Kodebox, Inc.
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

use super::asset::Asset;
use crate::CacheableItem;
use ccrypto::Blake;
use ckey::Address;
use ctypes::errors::RuntimeError;
use ctypes::{ShardId, Tracker};
use primitives::{H160, H256};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};

#[derive(Clone, Debug, PartialEq)]
pub struct AssetScheme {
    metadata: String,
    supply: u64,
    approver: Option<Address>,
    registrar: Option<Address>,
    allowed_script_hashes: Vec<H160>,
    pool: Vec<Asset>,
    seq: usize,
}

impl AssetScheme {
    pub fn new(
        metadata: String,
        supply: u64,
        approver: Option<Address>,
        registrar: Option<Address>,
        allowed_script_hashes: Vec<H160>,
    ) -> Self {
        Self {
            metadata,
            supply,
            approver,
            registrar,
            allowed_script_hashes,
            pool: Vec::new(),
            seq: 0,
        }
    }

    pub fn new_with_pool(
        metadata: String,
        supply: u64,
        approver: Option<Address>,
        registrar: Option<Address>,
        allowed_script_hashes: Vec<H160>,
        pool: Vec<Asset>,
    ) -> Self {
        Self {
            metadata,
            supply,
            approver,
            registrar,
            allowed_script_hashes,
            pool,
            seq: 0,
        }
    }

    pub fn metadata(&self) -> &String {
        &self.metadata
    }

    pub fn supply(&self) -> u64 {
        self.supply
    }

    pub fn approver(&self) -> &Option<Address> {
        &self.approver
    }

    pub fn registrar(&self) -> &Option<Address> {
        &self.registrar
    }

    pub fn allowed_script_hashes(&self) -> &[H160] {
        &self.allowed_script_hashes
    }

    pub fn seq(&self) -> usize {
        self.seq
    }

    pub fn is_permissioned(&self) -> bool {
        self.approver.is_some()
    }

    pub fn is_regulated(&self) -> bool {
        self.registrar.is_some()
    }

    pub fn is_allowed_script_hash(&self, lock_script_hash: &H160) -> bool {
        let allowed_hashes = self.allowed_script_hashes();
        allowed_hashes.is_empty() || allowed_hashes.contains(lock_script_hash)
    }

    pub fn pool(&self) -> &[Asset] {
        &self.pool
    }

    pub fn change_data(
        &mut self,
        metadata: String,
        approver: Option<Address>,
        registrar: Option<Address>,
        allowed_script_hashes: Vec<H160>,
    ) {
        self.metadata = metadata;
        self.approver = approver;
        self.registrar = registrar;
        self.allowed_script_hashes = allowed_script_hashes;
    }

    pub fn increase_supply(&mut self, quantity: u64) -> Result<u64, RuntimeError> {
        let headroom = std::u64::MAX - self.supply;
        if quantity > headroom {
            return Err(RuntimeError::AssetSupplyOverflow)
        }
        let previous = self.supply;
        self.supply += quantity;
        Ok(previous)
    }

    pub fn increase_seq(&mut self) {
        self.seq += 1;
    }

    pub fn reduce_supply(&mut self, quantity: u64) -> u64 {
        assert!(self.supply >= quantity, "AssetScheme supply shouldn't be depleted");
        let previous = self.supply;
        self.supply -= quantity;
        previous
    }
}

const PREFIX: u8 = super::ASSET_SCHEME_PREFIX;

impl Default for AssetScheme {
    fn default() -> Self {
        Self::new("".to_string(), 0, None, None, Vec::new())
    }
}

impl Encodable for AssetScheme {
    fn rlp_append(&self, s: &mut RlpStream) {
        if self.seq == 0 {
            s.begin_list(7);
        } else {
            s.begin_list(8);
        }
        s.append(&PREFIX)
            .append(&self.metadata)
            .append(&self.supply)
            .append(&self.approver)
            .append(&self.registrar)
            .append_list(&self.allowed_script_hashes)
            .append_list(&self.pool);
        if self.seq != 0 {
            s.append(&self.seq);
        }
    }
}

impl Decodable for AssetScheme {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let seq = match rlp.item_count()? {
            7 => 0,
            8 => rlp.val_at(7)?,
            item_count => {
                return Err(DecoderError::RlpInvalidLength {
                    got: item_count,
                    expected: 7,
                })
            }
        };

        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for asset scheme", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            metadata: rlp.val_at(1)?,
            supply: rlp.val_at(2)?,
            approver: rlp.val_at(3)?,
            registrar: rlp.val_at(4)?,
            allowed_script_hashes: rlp.list_at(5)?,
            pool: rlp.list_at(6)?,
            seq,
        })
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetSchemeAddress(H256);

impl_address!(SHARD, AssetSchemeAddress, PREFIX);

impl AssetSchemeAddress {
    pub fn new(asset_type: H160, shard_id: ShardId) -> Self {
        let index = ::std::u64::MAX;

        Self::from_hash_with_shard_id(asset_type, index, shard_id)
    }

    pub fn new_from_tracker(tracker: Tracker, shard_id: ShardId) -> Self {
        let asset_type = Blake::blake(*tracker);
        Self::new(asset_type, shard_id)
    }
}

impl CacheableItem for AssetScheme {
    type Address = AssetSchemeAddress;

    fn is_null(&self) -> bool {
        self.supply == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_from_address() {
        let origin = H160::random();
        let shard_id = 0xBEE;
        let asset_address = AssetSchemeAddress::new(origin, shard_id);
        let hash: H256 = asset_address.into();
        assert_eq!(hash[0..2], [PREFIX, 0]);
        assert_eq!(hash[2..4], [0x0B, 0xEE]); // shard id
    }

    #[test]
    fn shard_id() {
        let origin = H160::random();
        let shard_id = 0xCAA;
        let asset_scheme_address = AssetSchemeAddress::new(origin, shard_id);
        assert_eq!(shard_id, asset_scheme_address.shard_id());
    }

    #[test]
    fn shard_id_from_hash() {
        let hash = {
            let mut hash = H256::random();
            hash[0] = PREFIX;
            hash[1] = 0;
            hash
        };
        assert_eq!(::std::mem::size_of::<u16>(), ::std::mem::size_of::<ShardId>());
        let shard_id = (ShardId::from(hash[2]) << 8) + ShardId::from(hash[3]);
        let asset_scheme_address = AssetSchemeAddress::from_hash(hash).unwrap();
        assert_eq!(shard_id, asset_scheme_address.shard_id());
    }
}

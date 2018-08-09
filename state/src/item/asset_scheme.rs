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

use ckey::Address;
use ctypes::{ShardId, WorldId};
use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::cache::CacheableItem;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AssetScheme {
    metadata: String,
    amount: u64,
    registrar: Option<Address>,
}

impl AssetScheme {
    pub fn new(metadata: String, amount: u64, registrar: Option<Address>) -> Self {
        Self {
            metadata,
            amount,
            registrar,
        }
    }

    pub fn metadata(&self) -> &String {
        &self.metadata
    }

    pub fn amount(&self) -> &u64 {
        &self.amount
    }

    pub fn registrar(&self) -> &Option<Address> {
        &self.registrar
    }

    pub fn is_permissioned(&self) -> bool {
        self.registrar.is_some()
    }
}

const PREFIX: u8 = super::ASSET_SCHEME_PREFIX;

impl Encodable for AssetScheme {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(4).append(&PREFIX).append(&self.metadata).append(&self.amount).append(&self.registrar);
    }
}

impl Decodable for AssetScheme {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for asset scheme", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            metadata: rlp.val_at(1)?,
            amount: rlp.val_at(2)?,
            registrar: rlp.val_at(3)?,
        })
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetSchemeAddress(H256);

impl_address!(WORLD, AssetSchemeAddress, PREFIX);

impl AssetSchemeAddress {
    pub fn new(transaction_hash: H256, shard_id: ShardId, world_id: WorldId) -> Self {
        let index = ::std::u64::MAX;

        Self::from_transaction_hash_with_shard_and_world_id(transaction_hash, index, shard_id, world_id)
    }
}

impl CacheableItem for AssetScheme {
    type Address = AssetSchemeAddress;

    fn is_null(&self) -> bool {
        self.amount == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_from_address() {
        let origin = {
            let mut address;
            'address: loop {
                address = H256::random();
                if address[0] == 'S' as u8 {
                    continue
                }
                for i in 1..6 {
                    if address[i] == 0 {
                        continue 'address
                    }
                }
                break
            }
            address
        };
        let shard_id = 0xBEE;
        let world_id = 0xD00;
        let asset_address = AssetSchemeAddress::new(origin, shard_id, world_id);
        let hash: H256 = asset_address.into();
        assert_ne!(origin, hash);
        assert_eq!(hash[0..2], [PREFIX, 0]);
        assert_eq!(hash[2..4], [0x0B, 0xEE]); // shard id
        assert_eq!(hash[4..6], [0x0D, 0x00]); // world id
    }

    #[test]
    fn shard_id() {
        let origin = H256::random();
        let shard_id = 0xCAA;
        let world_id = 0xD0;
        let asset_scheme_address = AssetSchemeAddress::new(origin, shard_id, world_id);
        assert_eq!(shard_id, asset_scheme_address.shard_id());
    }

    #[test]
    fn world_id() {
        let origin = H256::random();
        let shard_id = 0xCAA;
        let world_id = 0xD0;
        let asset_scheme_address = AssetSchemeAddress::new(origin, shard_id, world_id);
        assert_eq!(world_id, asset_scheme_address.world_id());
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
        let shard_id = ((hash[2] as ShardId) << 8) + (hash[3] as ShardId);
        let asset_scheme_address = AssetSchemeAddress::from_hash(hash).unwrap();
        assert_eq!(shard_id, asset_scheme_address.shard_id());
    }


    #[test]
    fn world_id_from_hash() {
        let hash = {
            let mut hash = H256::random();
            hash[0] = PREFIX;
            hash[1] = 0;
            hash
        };
        assert_eq!(::std::mem::size_of::<u16>(), ::std::mem::size_of::<WorldId>());
        let world_id = ((hash[4] as WorldId) << 8) + (hash[5] as WorldId);
        let asset_scheme_address = AssetSchemeAddress::from_hash(hash).unwrap();
        assert_eq!(world_id, asset_scheme_address.world_id());
    }
}

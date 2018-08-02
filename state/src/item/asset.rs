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

use ctypes::ShardId;
use primitives::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::cache::CacheableItem;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    asset_type: H256,
    lock_script_hash: H256,
    parameters: Vec<Bytes>,
    amount: u64,
}

impl Asset {
    pub fn new(asset_type: H256, lock_script_hash: H256, parameters: Vec<Bytes>, amount: u64) -> Self {
        Self {
            asset_type,
            lock_script_hash,
            parameters,
            amount,
        }
    }

    pub fn asset_type(&self) -> &H256 {
        &self.asset_type
    }

    pub fn lock_script_hash(&self) -> &H256 {
        &self.lock_script_hash
    }

    pub fn parameters(&self) -> &Vec<Bytes> {
        &self.parameters
    }

    pub fn amount(&self) -> &u64 {
        &self.amount
    }
}

impl CacheableItem for Asset {
    type Address = AssetAddress;

    fn is_null(&self) -> bool {
        self.amount == 0
    }
}

const PREFIX: u8 = 'A' as u8;

impl Encodable for Asset {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(5)
            .append(&PREFIX)
            .append(&self.asset_type)
            .append(&self.lock_script_hash)
            .append(&self.parameters)
            .append(&self.amount);
    }
}

impl Decodable for Asset {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for asset", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            asset_type: rlp.val_at(1)?,
            lock_script_hash: rlp.val_at(2)?,
            parameters: rlp.val_at(3)?,
            amount: rlp.val_at(4)?,
        })
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetAddress(H256);

impl_address!(SHARD, AssetAddress, PREFIX);

impl AssetAddress {
    pub fn new(transaction_hash: H256, index: usize, shard_id: ShardId) -> Self {
        debug_assert_eq!(::std::mem::size_of::<u64>(), ::std::mem::size_of::<usize>());
        let index = index as u64;

        Self::from_transaction_hash_with_shard_id(transaction_hash, index, shard_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_from_address() {
        let parcel_id = {
            let mut address;
            'address: loop {
                address = H256::random();
                if address[0] == PREFIX {
                    continue
                }
                for i in 1..8 {
                    if address[i] == 0 {
                        continue 'address
                    }
                }
                break
            }
            address
        };
        let shard_id = 0xBeef;
        let address1 = AssetAddress::new(parcel_id, 0, shard_id);
        let address2 = AssetAddress::new(parcel_id, 1, shard_id);
        assert_ne!(address1, address2);
        assert_eq!(address1[0..2], [PREFIX, 0]);
        assert_eq!(address1[2..4], [0xBE, 0xEF]); // shard id
        assert_eq!(address1[4..6], [0, 0]); // world id
        assert_eq!(address2[0..2], [PREFIX, 0]);
        assert_eq!(address2[2..4], [0xBE, 0xEF]); // shard id
        assert_eq!(address2[4..6], [0, 0]); // world id
    }

    #[test]
    fn parse_fail_return_none() {
        let hash = {
            let mut hash;
            loop {
                hash = H256::random();
                if hash[0] == PREFIX {
                    continue
                }
                for i in 1..6 {
                    if hash[i] == 0 {
                        continue
                    }
                }
                break
            }
            hash
        };
        let address = AssetAddress::from_hash(hash);
        assert!(address.is_none());
    }

    #[test]
    fn parse_return_some() {
        let hash = {
            let mut hash = H256::random();
            hash[0..6].clone_from_slice(&[PREFIX, 0, 0, 0, 0, 0]);
            hash
        };
        let address = AssetAddress::from_hash(hash.clone());
        assert_eq!(Some(AssetAddress(hash)), address);
    }

    #[test]
    fn shard_id() {
        let origin = H256::random();
        let shard_id = 0xCAA;
        let asset_address = AssetAddress::new(origin, 2, shard_id);
        assert_eq!(shard_id, asset_address.shard_id());
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
        let asset_address = AssetAddress::from_hash(hash).unwrap();
        assert_eq!(shard_id, asset_address.shard_id());
    }
}

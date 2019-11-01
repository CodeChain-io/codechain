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

use ctypes::{ShardId, Tracker};
use primitives::{Bytes, H160, H256};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};

use crate::CacheableItem;

#[derive(Clone, Debug, PartialEq, RlpEncodable, RlpDecodable)]
pub struct Asset {
    asset_type: H160,
    quantity: u64,
}

impl Asset {
    pub fn new(asset_type: H160, quantity: u64) -> Self {
        Self {
            asset_type,
            quantity,
        }
    }

    pub fn asset_type(&self) -> &H160 {
        &self.asset_type
    }

    pub fn quantity(&self) -> u64 {
        self.quantity
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedAsset {
    asset: Asset,
    lock_script_hash: H160,
    parameters: Vec<Bytes>,
}

impl OwnedAsset {
    pub fn new(asset_type: H160, lock_script_hash: H160, parameters: Vec<Bytes>, quantity: u64) -> Self {
        Self {
            asset: Asset {
                asset_type,
                quantity,
            },
            lock_script_hash,
            parameters,
        }
    }

    pub fn asset_type(&self) -> &H160 {
        &self.asset.asset_type()
    }

    pub fn lock_script_hash(&self) -> &H160 {
        &self.lock_script_hash
    }

    pub fn parameters(&self) -> &Vec<Bytes> {
        &self.parameters
    }

    pub fn quantity(&self) -> u64 {
        self.asset.quantity()
    }
}

impl Default for OwnedAsset {
    fn default() -> Self {
        Self {
            asset: Asset {
                asset_type: H160::zero(),
                quantity: 0,
            },
            lock_script_hash: H160::zero(),
            parameters: vec![],
        }
    }
}

impl CacheableItem for OwnedAsset {
    type Address = OwnedAssetAddress;

    fn is_null(&self) -> bool {
        self.asset.quantity() == 0
    }
}

const PREFIX: u8 = super::OWNED_ASSET_PREFIX;

impl Encodable for OwnedAsset {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(6)
            .append(&PREFIX)
            .append(self.asset.asset_type())
            .append(&self.asset.quantity())
            .append(&self.lock_script_hash)
            .append(&self.parameters)
            // NOTE: The order_hash field removed.
            .append(&Option::<H256>::None);
    }
}

impl Decodable for OwnedAsset {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let item_count = rlp.item_count()?;
        if rlp.item_count()? != 6 {
            return Err(DecoderError::RlpInvalidLength {
                expected: 6,
                got: item_count,
            })
        }

        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for asset", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        let order_hash = rlp.val_at::<Option<H256>>(5)?;
        if let Some(h) = order_hash {
            cdebug!(STATE, "order_hash must be None but Some({}) is given", h);
            return Err(DecoderError::Custom("order_hash must be None"))
        }
        Ok(Self {
            asset: Asset {
                asset_type: rlp.val_at(1)?,
                quantity: rlp.val_at(2)?,
            },
            lock_script_hash: rlp.val_at(3)?,
            parameters: rlp.val_at(4)?,
        })
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OwnedAssetAddress(H256);

impl_address!(SHARD, OwnedAssetAddress, PREFIX);

impl OwnedAssetAddress {
    pub fn new(tracker: Tracker, index: usize, shard_id: ShardId) -> Self {
        debug_assert_eq!(::std::mem::size_of::<u64>(), ::std::mem::size_of::<usize>());
        let index = index as u64;

        Self::from_hash_with_shard_id(*tracker, index, shard_id)
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn asset_from_address() {
        let tracker = {
            let mut address;
            'address: loop {
                address = H256::random();
                if address[0] == PREFIX {
                    continue
                }
                for a in address.iter().take(8).skip(1) {
                    if *a == 0 {
                        continue 'address
                    }
                }
                break
            }
            address.into()
        };
        let shard_id = 0xBEEF;
        let address1 = OwnedAssetAddress::new(tracker, 0, shard_id);
        let address2 = OwnedAssetAddress::new(tracker, 1, shard_id);
        assert_ne!(address1, address2);
        assert_eq!(address1[0..2], [PREFIX, 0]);
        assert_eq!(address1[2..4], [0xBE, 0xEF]); // shard id
        assert_eq!(address2[0..2], [PREFIX, 0]);
        assert_eq!(address2[2..4], [0xBE, 0xEF]); // shard id
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
                for h in hash.iter().take(6).skip(1) {
                    if *h == 0 {
                        continue
                    }
                }
                break
            }
            hash
        };
        let address = OwnedAssetAddress::from_hash(hash);
        assert!(address.is_none());
    }

    #[test]
    fn parse_return_some() {
        let hash = {
            let mut hash = H256::random();
            hash[0..6].copy_from_slice(&[PREFIX, 0, 0, 0, 0, 0]);
            hash
        };
        let address = OwnedAssetAddress::from_hash(hash);
        assert_eq!(Some(OwnedAssetAddress(hash)), address);
    }

    #[test]
    fn shard_id() {
        let origin = H256::random().into();
        let shard_id = 0xCAA;
        let asset_address = OwnedAssetAddress::new(origin, 2, shard_id);
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
        let shard_id = (ShardId::from(hash[2]) << 8) + ShardId::from(hash[3]);
        let asset_address = OwnedAssetAddress::from_hash(hash).unwrap();
        assert_eq!(shard_id, asset_address.shard_id());
    }

    #[test]
    fn encode_and_decode_asset() {
        rlp_encode_and_decode_test!(Asset {
            asset_type: H160::random(),
            quantity: 0,
        });
    }
}

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
use primitives::{Bytes, H160, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use crate::CacheableItem;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, RlpEncodable, RlpDecodable)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    asset_type: H256,
    amount: u64,
}

impl Asset {
    pub fn new(asset_type: H256, amount: u64) -> Self {
        Self {
            asset_type,
            amount,
        }
    }

    pub fn asset_type(&self) -> &H256 {
        &self.asset_type
    }

    pub fn amount(&self) -> u64 {
        self.amount
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OwnedAsset {
    #[serde(flatten)]
    asset: Asset,
    lock_script_hash: H160,
    parameters: Vec<Bytes>,
    order_hash: Option<H256>,
}

impl OwnedAsset {
    pub fn new(
        asset_type: H256,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        amount: u64,
        order_hash: Option<H256>,
    ) -> Self {
        Self {
            asset: Asset {
                asset_type,
                amount,
            },
            lock_script_hash,
            parameters,
            order_hash,
        }
    }

    pub fn asset_type(&self) -> &H256 {
        &self.asset.asset_type()
    }

    pub fn lock_script_hash(&self) -> &H160 {
        &self.lock_script_hash
    }

    pub fn parameters(&self) -> &Vec<Bytes> {
        &self.parameters
    }

    pub fn amount(&self) -> u64 {
        self.asset.amount()
    }

    pub fn order_hash(&self) -> &Option<H256> {
        &self.order_hash
    }

    pub fn init(
        &mut self,
        asset_type: H256,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        amount: u64,
        order_hash: Option<H256>,
    ) {
        assert_eq!(
            Asset {
                asset_type: H256::zero(),
                amount: 0
            },
            self.asset
        );
        assert_eq!(H160::zero(), self.lock_script_hash);
        assert_eq!(0, self.parameters.len());
        self.asset = Asset {
            asset_type,
            amount,
        };
        self.lock_script_hash = lock_script_hash;
        self.parameters = parameters;
        self.order_hash = order_hash;
    }
}

impl Default for OwnedAsset {
    fn default() -> Self {
        Self {
            asset: Asset {
                asset_type: H256::zero(),
                amount: 0,
            },
            lock_script_hash: H160::zero(),
            parameters: vec![],
            order_hash: None,
        }
    }
}

impl CacheableItem for OwnedAsset {
    type Address = OwnedAssetAddress;

    fn is_null(&self) -> bool {
        self.asset.amount() == 0
    }
}

const PREFIX: u8 = super::OWNED_ASSET_PREFIX;

impl Encodable for OwnedAsset {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(6)
            .append(&PREFIX)
            .append(self.asset.asset_type())
            .append(&self.asset.amount())
            .append(&self.lock_script_hash)
            .append(&self.parameters)
            .append(&self.order_hash);
    }
}

impl Decodable for OwnedAsset {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 6 {
            return Err(DecoderError::RlpInvalidLength)
        }

        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for asset", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            asset: Asset {
                asset_type: rlp.val_at(1)?,
                amount: rlp.val_at(2)?,
            },
            lock_script_hash: rlp.val_at(3)?,
            parameters: rlp.val_at(4)?,
            order_hash: rlp.val_at(5)?,
        })
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OwnedAssetAddress(H256);

impl_address!(SHARD, OwnedAssetAddress, PREFIX);

impl OwnedAssetAddress {
    pub fn new(transaction_hash: H256, index: usize, shard_id: ShardId) -> Self {
        debug_assert_eq!(::std::mem::size_of::<u64>(), ::std::mem::size_of::<usize>());
        let index = index as u64;

        Self::from_transaction_hash_with_shard_id(transaction_hash, index, shard_id)
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

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
                for a in address.iter().take(8).skip(1) {
                    if *a == 0 {
                        continue 'address
                    }
                }
                break
            }
            address
        };
        let shard_id = 0xBEEF;
        let address1 = OwnedAssetAddress::new(parcel_id, 0, shard_id);
        let address2 = OwnedAssetAddress::new(parcel_id, 1, shard_id);
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
            hash[0..6].clone_from_slice(&[PREFIX, 0, 0, 0, 0, 0]);
            hash
        };
        let address = OwnedAssetAddress::from_hash(hash);
        assert_eq!(Some(OwnedAssetAddress(hash)), address);
    }

    #[test]
    fn shard_id() {
        let origin = H256::random();
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
            asset_type: H256::random(),
            amount: 0,
        });
    }

}

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

use ctypes::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::CacheableItem;

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

    fn overwrite_with(&mut self, other: Self) {
        self.asset_type = other.asset_type;
        self.lock_script_hash = other.lock_script_hash;
        self.parameters = other.parameters;
        self.amount = other.amount;
    }

    fn is_null(&self) -> bool {
        self.amount == 0
    }

    fn from_rlp(rlp: &[u8]) -> Self {
        ::rlp::decode(rlp)
    }

    fn rlp(&self) -> Bytes {
        ::rlp::encode(self).into_vec()
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

impl_address!(AssetAddress, PREFIX);

impl AssetAddress {
    pub fn new(transaction_hash: H256, index: usize) -> Self {
        debug_assert_eq!(::std::mem::size_of::<u64>(), ::std::mem::size_of::<usize>());
        let index = index as u64;

        Self::from_transaction_hash(transaction_hash, index)
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetAddress, H256, PREFIX};

    #[test]
    fn asset_from_address() {
        let parcel_id = {
            let mut address;
            loop {
                address = H256::random();
                if address[0] == PREFIX {
                    continue
                }
                for i in 1..8 {
                    if address[i] == 0 {
                        continue
                    }
                }
                break
            }
            address
        };
        let address1 = AssetAddress::new(parcel_id, 0);
        let address2 = AssetAddress::new(parcel_id, 1);
        assert_ne!(address1, address2);
        assert_eq!(address1[0..8], [PREFIX, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(address2[0..8], [PREFIX, 0, 0, 0, 0, 0, 0, 0]);
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
                for i in 1..8 {
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
            hash[0..8].clone_from_slice(&[PREFIX, 0, 0, 0, 0, 0, 0, 0]);
            hash
        };
        let address = AssetAddress::from_hash(hash.clone());
        assert_eq!(Some(AssetAddress(hash)), address);
    }
}

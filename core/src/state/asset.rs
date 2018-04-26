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

use std::fmt;
use std::ops::Deref;

use cbytes::Bytes;
use ccrypto::blake256_with_key;
use ctypes::{H128, H256, U256};

use super::CacheableItem;

#[derive(Clone, Debug, RlpEncodable, RlpDecodable, Serialize, Deserialize)]
pub struct Asset {
    asset_type: H256,
    lock_script: H256,
    parameters: Vec<Bytes>,
    amount: U256,
}

impl Asset {
    pub fn new(asset_type: H256, lock_script: H256, parameters: Vec<Bytes>, amount: U256) -> Self {
        Self {
            asset_type,
            lock_script,
            parameters,
            amount,
        }
    }

    pub fn asset_type(&self) -> &H256 {
        &self.asset_type
    }

    pub fn lock_script(&self) -> &H256 {
        &self.lock_script
    }

    pub fn parameters(&self) -> &Vec<Bytes> {
        &self.parameters
    }

    pub fn amount(&self) -> &U256 {
        &self.amount
    }
}

impl CacheableItem for Asset {
    type Address = AssetAddress;

    fn overwrite_with(&mut self, other: Self) {
        self.asset_type = other.asset_type;
        self.lock_script = other.lock_script;
        self.parameters = other.parameters;
        self.amount = other.amount;
    }

    fn is_null(&self) -> bool {
        self.amount.is_zero()
    }

    fn from_rlp(rlp: &[u8]) -> Self {
        ::rlp::decode(rlp)
    }

    fn rlp(&self) -> Bytes {
        ::rlp::encode(self).into_vec()
    }
}

impl AssetAddress {
    pub fn new(txhash: H256, index: usize) -> Self {
        debug_assert_eq!(::std::mem::size_of::<u64>(), ::std::mem::size_of::<usize>());

        let mut hash = blake256_with_key(&txhash, &H128::from(index as u64));
        hash[0..8].clone_from_slice(&['A' as u8, 0, 0, 0, 0, 0, 0, 0]);
        AssetAddress(hash)
    }

    pub fn from_hash(hash: H256) -> Option<Self> {
        if hash[0..8] == ['A' as u8, 0, 0, 0, 0, 0, 0, 0] {
            Some(AssetAddress(hash))
        } else {
            None
        }
    }
}

impl Into<H256> for AssetAddress {
    fn into(self) -> H256 {
        self.0
    }
}

impl<'a> Into<&'a H256> for &'a AssetAddress {
    fn into(self) -> &'a H256 {
        &self.0
    }
}

impl fmt::Debug for AssetAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for AssetAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for AssetAddress {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &(*&self.0)
    }
}
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetAddress(H256);

#[cfg(test)]
mod tests {
    use super::{AssetAddress, H256};

    #[test]
    fn asset_from_address() {
        let transaction_id = {
            let mut address = Default::default();
            loop {
                address = H256::random();
                if address[0] == 'A' as u8 {
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
        let address1 = AssetAddress::new(transaction_id, 0);
        let address2 = AssetAddress::new(transaction_id, 1);
        assert_ne!(address1, address2);
        assert_eq!(address1[0..8], ['A' as u8, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(address2[0..8], ['A' as u8, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn parse_fail_return_none() {
        let hash = {
            let mut hash = Default::default();
            loop {
                hash = H256::random();
                if hash[0] == 'A' as u8 {
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
            hash[0..8].clone_from_slice(&['A' as u8, 0, 0, 0, 0, 0, 0, 0]);
            hash
        };
        let address = AssetAddress::from_hash(hash.clone());
        assert_eq!(Some(AssetAddress(hash)), address);
    }
}

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
use ctypes::hash::H256;
use ctypes::Address;

#[derive(Clone, Debug, RlpEncodable, RlpDecodable)]
pub struct Asset {
    metadata: String,
    registrar: Address,
    permissioned: bool,
}

impl Asset {
    pub fn new(metadata: String, registrar: Address, permissioned: bool) -> Self {
        Self {
            metadata,
            registrar,
            permissioned,
        }
    }

    pub fn from_rlp(rlp: &[u8]) -> Self {
        ::rlp::decode(rlp)
    }

    pub fn rlp(&self) -> Bytes {
        ::rlp::encode(self).into_vec()
    }

    pub fn metadata(&self) -> &String {
        &self.metadata
    }

    pub fn registrar(&self) -> &Address {
        &self.registrar
    }

    pub fn permissioned(&self) -> bool {
        self.permissioned
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetAddress(H256);

impl AssetAddress {
    fn new(hash: H256) -> Self {
        debug_assert_eq!(['A' as u8, 0, 0, 0, 0, 0, 0, 0], hash[0..8]);
        AssetAddress(hash)
    }
}

impl From<H256> for AssetAddress {
    fn from(mut hash: H256) -> Self {
        hash[0..8].clone_from_slice(&['A' as u8, 0, 0, 0, 0, 0, 0, 0]);
        Self::new(hash)
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

#[cfg(test)]
mod tests {
    use super::{AssetAddress, H256};

    #[test]
    fn asset_from_address() {
        let origin = {
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
        let asset_address = AssetAddress::from(origin);
        let hash: H256 = asset_address.into();
        assert_ne!(origin, hash);
        assert_eq!(hash[0..8], ['A' as u8, 0, 0, 0, 0, 0, 0, 0]);
    }
}

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

use super::CacheableItem;

#[derive(Clone, Debug, RlpEncodable, RlpDecodable, Serialize, Deserialize)]
pub struct AssetScheme {
    metadata: String,
    registrar: Address,
    permissioned: bool,
}

impl AssetScheme {
    pub fn new(metadata: String, registrar: Address, permissioned: bool) -> Self {
        Self {
            metadata,
            registrar,
            permissioned,
        }
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
pub struct AssetSchemeAddress(H256);

impl AssetSchemeAddress {
    fn new(hash: H256) -> Self {
        debug_assert_eq!(['S' as u8, 0, 0, 0], hash[0..4]);
        debug_assert_eq!([0, 0, 0, 0], hash[4..8]); // world id
        AssetSchemeAddress(hash)
    }
}

impl From<H256> for AssetSchemeAddress {
    fn from(mut hash: H256) -> Self {
        hash[0..8].clone_from_slice(&['S' as u8, 0, 0, 0, 0, 0, 0, 0]);
        Self::new(hash)
    }
}

impl Into<H256> for AssetSchemeAddress {
    fn into(self) -> H256 {
        self.0
    }
}

impl<'a> Into<&'a H256> for &'a AssetSchemeAddress {
    fn into(self) -> &'a H256 {
        &self.0
    }
}

impl fmt::Debug for AssetSchemeAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for AssetSchemeAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for AssetSchemeAddress {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &(*&self.0)
    }
}

impl CacheableItem for AssetScheme {
    type Address = AssetSchemeAddress;
    fn overwrite_with(&mut self, other: Self) {
        self.metadata = other.metadata;
        self.registrar = other.registrar;
        self.permissioned = other.permissioned;
    }

    fn is_null(&self) -> bool {
        false
    }

    fn from_rlp(rlp: &[u8]) -> Self {
        ::rlp::decode(rlp)
    }

    fn rlp(&self) -> Bytes {
        ::rlp::encode(self).into_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetSchemeAddress, H256};

    #[test]
    fn asset_from_address() {
        let origin = {
            let mut address = Default::default();
            loop {
                address = H256::random();
                if address[0] == 'S' as u8 {
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
        let asset_address = AssetSchemeAddress::from(origin);
        let hash: H256 = asset_address.into();
        assert_ne!(origin, hash);
        assert_eq!(hash[0..4], ['S' as u8, 0, 0, 0]);
        assert_eq!(hash[4..8], [0, 0, 0, 0]); // world id
        assert_eq!(origin[8..32], hash[8..32]);
    }
}

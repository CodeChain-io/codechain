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

macro_rules! impl_address {
    ($name:ident, $prefix:expr) => {
        impl $name {
            fn from_parcel_hash(parcel_hash: ::ctypes::H256, index: u64) -> Self {
                let mut hash = ::ccrypto::blake256_with_key(&parcel_hash, &::ctypes::H128::from(index));
                hash[0..8].clone_from_slice(&[$prefix, 0, 0, 0, 0, 0, 0, 0]);
                $name(hash)
            }

            pub fn from_hash(hash: ::ctypes::H256) -> Option<Self> {
                if Self::is_valid_format(&hash) {
                    Some($name(hash))
                } else {
                    None
                }
            }

            pub fn is_valid_format(hash: &::ctypes::H256) -> bool {
                if hash[0..4] != [$prefix, 0, 0, 0] {
                    return false // prefix
                }
                hash[4..8] == [0, 0, 0, 0] // world id
            }
        }

        impl Into<::ctypes::H256> for $name {
            fn into(self) -> ::ctypes::H256 {
                self.0
            }
        }

        impl<'a> Into<&'a ::ctypes::H256> for &'a $name {
            fn into(self) -> &'a ::ctypes::H256 {
                &self.0
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }

        impl ::std::ops::Deref for $name {
            type Target = [u8];

            #[inline]
            fn deref(&self) -> &Self::Target {
                &(*&self.0)
            }
        }
    };
}

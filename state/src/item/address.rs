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

macro_rules! define_address_constructor {
    (TOP, $name:ident, $prefix:expr) => {
        fn from_transaction_hash(transaction_hash: ::primitives::H256, index: u64) -> Self {
            let h248: ::primitives::H248 =
                ::ccrypto::Blake::blake_with_key(&transaction_hash, &::primitives::H128::from(index));
            let mut hash: H256 = h248.into();
            hash[0..1].clone_from_slice(&[$prefix]);
            $name(hash)
        }
    };
    (SHARD, $name:ident, $prefix:expr) => {
        fn from_transaction_hash_with_shard_id(
            transaction_hash: ::primitives::H256,
            index: u64,
            shard_id: ::ctypes::ShardId,
        ) -> Self {
            let mut hash: ::primitives::H256 =
                ::ccrypto::Blake::blake_with_key(&transaction_hash, &::primitives::H128::from(index));
            hash[0..2].clone_from_slice(&[$prefix, 0]);

            let mut shard_id_bytes = Vec::<u8>::new();
            debug_assert_eq!(::std::mem::size_of::<u16>(), ::std::mem::size_of::<::ctypes::ShardId>());
            ::byteorder::WriteBytesExt::write_u16::<::byteorder::BigEndian>(&mut shard_id_bytes, shard_id).unwrap();
            assert_eq!(2, shard_id_bytes.len());
            hash[2..4].clone_from_slice(&shard_id_bytes);

            hash[4..6].clone_from_slice(&[0, 0]); // world id

            $name(hash)
        }
    };
}

macro_rules! define_shard_id {
    (TOP) => {
    };
    (SHARD) => {
        pub fn shard_id(&self) -> ::ctypes::ShardId {
            debug_assert_eq!(::std::mem::size_of::<u16>(), ::std::mem::size_of::<ShardId>());
            use byteorder::ReadBytesExt;
            ::std::io::Cursor::new(&self.0[2..4]).read_u16::<::byteorder::BigEndian>().unwrap()
        }
    };
}

macro_rules! impl_address {
    ($type:ident, $name:ident, $prefix:expr) => {
        impl $name {
            define_address_constructor!($type, $name, $prefix);

            define_shard_id!($type);

            pub fn from_hash(hash: ::primitives::H256) -> Option<Self> {
                if Self::is_valid_format(&hash) {
                    Some($name(hash))
                } else {
                    None
                }
            }

            pub fn is_valid_format(hash: &::primitives::H256) -> bool {
                hash[0] == $prefix
            }
        }

        impl Into<::primitives::H256> for $name {
            fn into(self) -> ::primitives::H256 {
                self.0
            }
        }

        impl<'a> Into<&'a ::primitives::H256> for &'a $name {
            fn into(self) -> &'a ::primitives::H256 {
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

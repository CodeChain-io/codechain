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

use std::mem::size_of;

use byteorder::{BigEndian, WriteBytesExt};
use ckey::Address;
use ctypes::ShardId;
use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::super::CacheableItem;
use super::asset::Asset;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetScheme {
    metadata: String,
    amount: u64,
    registrar: Option<Address>,
    pool: Vec<Asset>,
}

impl AssetScheme {
    pub fn new(metadata: String, amount: u64, registrar: Option<Address>) -> Self {
        Self {
            metadata,
            amount,
            registrar,
            pool: Vec::new(),
        }
    }

    pub fn new_with_pool(metadata: String, amount: u64, registrar: Option<Address>, pool: Vec<Asset>) -> Self {
        Self {
            metadata,
            amount,
            registrar,
            pool,
        }
    }

    pub fn metadata(&self) -> &String {
        &self.metadata
    }

    pub fn amount(&self) -> u64 {
        self.amount
    }

    pub fn registrar(&self) -> &Option<Address> {
        &self.registrar
    }

    pub fn is_permissioned(&self) -> bool {
        self.registrar.is_some()
    }

    pub fn init(&mut self, metadata: String, amount: u64, registrar: Option<Address>, pool: Vec<Asset>) {
        assert_eq!("", &self.metadata);
        assert_eq!(0, self.amount);
        assert_eq!(None, self.registrar);
        self.metadata = metadata;
        self.amount = amount;
        self.registrar = registrar;
        self.pool = pool;
    }

    pub fn pool(&self) -> &[Asset] {
        &self.pool
    }
}

const PREFIX: u8 = super::ASSET_SCHEME_PREFIX;

impl Default for AssetScheme {
    fn default() -> Self {
        Self::new("".to_string(), 0, None)
    }
}

impl Encodable for AssetScheme {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(5)
            .append(&PREFIX)
            .append(&self.metadata)
            .append(&self.amount)
            .append(&self.registrar)
            .append_list(&self.pool);
    }
}

impl Decodable for AssetScheme {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 5 {
            return Err(DecoderError::RlpInvalidLength)
        }

        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for asset scheme", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            metadata: rlp.val_at(1)?,
            amount: rlp.val_at(2)?,
            registrar: rlp.val_at(3)?,
            pool: rlp.list_at(4)?,
        })
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetSchemeAddress(H256);

impl_address!(SHARD, AssetSchemeAddress, PREFIX);

impl AssetSchemeAddress {
    pub fn new(transaction_hash: H256, shard_id: ShardId) -> Self {
        let index = ::std::u64::MAX;

        Self::from_transaction_hash_with_shard_id(transaction_hash, index, shard_id)
    }
    pub fn new_with_zero_suffix(shard_id: ShardId) -> Self {
        let mut hash = H256::zero();
        hash[0..2].clone_from_slice(&[PREFIX, 0]);

        let mut shard_id_bytes = Vec::<u8>::new();
        debug_assert_eq!(size_of::<u16>(), size_of::<ShardId>());
        WriteBytesExt::write_u16::<BigEndian>(&mut shard_id_bytes, shard_id).unwrap();
        assert_eq!(2, shard_id_bytes.len());
        hash[2..4].clone_from_slice(&shard_id_bytes);

        AssetSchemeAddress(hash)
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
        let asset_address = AssetSchemeAddress::new(origin, shard_id);
        let hash: H256 = asset_address.into();
        assert_ne!(origin, hash);
        assert_eq!(hash[0..2], [PREFIX, 0]);
        assert_eq!(hash[2..4], [0x0B, 0xEE]); // shard id
    }

    #[test]
    fn shard_id() {
        let origin = H256::random();
        let shard_id = 0xCAA;
        let asset_scheme_address = AssetSchemeAddress::new(origin, shard_id);
        assert_eq!(shard_id, asset_scheme_address.shard_id());
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
}

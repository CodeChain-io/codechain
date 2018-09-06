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

use ctypes::{ShardId, WorldId};
use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::local_cache::CacheableItem;

#[derive(Clone, Debug, PartialEq)]
pub struct ShardMetadata {
    number_of_worlds: WorldId,
    nonce: u64,
}

impl ShardMetadata {
    pub fn new(number_of_worlds: WorldId) -> Self {
        Self {
            number_of_worlds,
            nonce: 0,
        }
    }

    pub fn new_with_nonce(number_of_worlds: WorldId, nonce: u64) -> Self {
        Self {
            number_of_worlds,
            nonce,
        }
    }

    pub fn increase_number_of_worlds(&mut self) {
        self.number_of_worlds += 1;
    }

    pub fn number_of_worlds(&self) -> &WorldId {
        &self.number_of_worlds
    }

    pub fn increase_nonce(&mut self) {
        self.nonce += 1;
    }

    pub fn nonce(&self) -> &u64 {
        &self.nonce
    }
}

impl Default for ShardMetadata {
    fn default() -> Self {
        Self {
            number_of_worlds: 0,
            nonce: 0,
        }
    }
}

impl CacheableItem for ShardMetadata {
    type Address = ShardMetadataAddress;

    fn is_null(&self) -> bool {
        self.nonce == 0
    }
}

const PREFIX: u8 = super::SHARD_METADATA_PREFIX;

impl Encodable for ShardMetadata {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3).append(&PREFIX).append(&self.number_of_worlds).append(&self.nonce);
    }
}

impl Decodable for ShardMetadata {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 3 {
            return Err(DecoderError::RlpInvalidLength)
        }
        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for asset", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            number_of_worlds: rlp.val_at(1)?,
            nonce: rlp.val_at(2)?,
        })
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ShardMetadataAddress(H256);

impl_address!(SHARD, ShardMetadataAddress, PREFIX);

impl ShardMetadataAddress {
    pub fn new(shard_id: ShardId) -> Self {
        Self::from_transaction_hash_with_shard_id(H256::from_slice(b"metadata address"), shard_id as u64, shard_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_id_makes_different_address() {
        let address1 = ShardMetadataAddress::new(0);
        let address2 = ShardMetadataAddress::new(1);
        assert_ne!(address1, address2);
        assert_eq!(address1[0..2], [PREFIX, 0]);
        assert_eq!(address1[2..4], [0, 0]); // shard id
        assert_eq!(address1[4..6], [0, 0]); // world id
        assert_eq!(address2[0..2], [PREFIX, 0]);
        assert_eq!(address2[2..4], [0, 1]); // shard id
        assert_eq!(address2[4..6], [0, 0]); // world id
    }

    #[test]
    fn parse_fail_return_none() {
        let hash = {
            let mut hash;
            'hash: loop {
                hash = H256::random();
                if hash[0] == PREFIX {
                    continue
                }
                for i in 1..6 {
                    if hash[i] == 0 {
                        continue 'hash
                    }
                }
                break
            }
            hash
        };
        let address = ShardMetadataAddress::from_hash(hash);
        assert!(address.is_none());
    }

    #[test]
    fn parse_return_some() {
        let hash = {
            let mut hash = H256::random();
            hash[0..6].clone_from_slice(&[PREFIX, 0, 0, 0, 0, 0]);
            hash
        };
        let address = ShardMetadataAddress::from_hash(hash.clone());
        assert_eq!(Some(ShardMetadataAddress(hash)), address);
    }

    #[test]
    fn shard_id() {
        let shard_id = 0xCAA;
        let address = ShardMetadataAddress::new(shard_id);
        assert_eq!(shard_id, address.shard_id());
    }

    #[test]
    fn shard_id_from_hash() {
        let hash = {
            let mut hash = H256::random();
            hash[0] = PREFIX;
            hash[1] = 0;
            hash
        };
        let shard_id = ((hash[2] as ShardId) << 8) + (hash[3] as ShardId);
        let address = ShardMetadataAddress::from_hash(hash).unwrap();
        assert_eq!(shard_id, address.shard_id());
    }
}

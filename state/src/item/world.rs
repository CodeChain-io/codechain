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

use ckey::Address;
use ctypes::{ShardId, WorldId};
use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::cache::CacheableItem;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct World {
    owners: Vec<Address>,
    users: Vec<Address>,
    nonce: u64,
}

impl World {
    pub fn new(owners: Vec<Address>, users: Vec<Address>) -> Self {
        Self {
            owners,
            users,
            nonce: 0,
        }
    }

    pub fn new_with_nonce(owners: Vec<Address>, users: Vec<Address>, nonce: u64) -> Self {
        Self {
            owners,
            users,
            nonce,
        }
    }

    pub fn owners(&self) -> &[Address] {
        &self.owners
    }

    pub fn users(&self) -> &[Address] {
        &self.users
    }

    pub fn nonce(&self) -> &u64 {
        &self.nonce
    }

    pub fn inc_nonce(&mut self) {
        debug_assert_ne!(::std::u64::MAX, self.nonce);
        self.nonce += 1;
    }

    pub fn set_owners(&mut self, owners: Vec<Address>) {
        debug_assert_ne!(Vec::<Address>::new(), owners);
        self.owners = owners;
    }

    pub fn set_users(&mut self, users: Vec<Address>) {
        self.users = users;
    }
}

impl CacheableItem for World {
    type Address = WorldAddress;

    fn is_null(&self) -> bool {
        self.owners.is_empty() && self.nonce == 0
    }
}

const PREFIX: u8 = super::WORLD_PREFIX;

impl Encodable for World {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(4).append(&PREFIX).append_list(self.owners()).append_list(self.users()).append(self.nonce());
    }
}

impl Decodable for World {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 4 {
            return Err(DecoderError::RlpInvalidLength)
        }
        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for world", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            owners: rlp.list_at(1)?,
            users: rlp.list_at(2)?,
            nonce: rlp.val_at(3)?,
        })
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorldAddress(H256);

impl_address!(WORLD, WorldAddress, PREFIX);

impl WorldAddress {
    pub fn new(shard_id: ShardId, world_id: WorldId) -> Self {
        Self::from_transaction_hash_with_shard_and_world_id(H256::from_slice(b"world"), 0, shard_id, world_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address() {
        let shard_id = 0xBeef;
        let world_id = 0xCafe;
        let address = WorldAddress::new(shard_id, world_id);
        assert_eq!(address[0..2], [PREFIX, 0]);
        assert_eq!(address[2..4], [0xBE, 0xEF]); // shard id
        assert_eq!(address[4..6], [0xCA, 0xFE]); // world id
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
                for i in 1..6 {
                    if hash[i] == 0 {
                        continue
                    }
                }
                break
            }
            hash
        };
        let address = WorldAddress::from_hash(hash);
        assert_eq!(None, address);
    }

    #[test]
    fn parse_return_some() {
        let hash = {
            let mut hash = H256::random();
            hash[0..6].clone_from_slice(&[PREFIX, 0, 0, 0, 0, 0]);
            hash
        };
        let address = WorldAddress::from_hash(hash.clone());
        assert_eq!(Some(WorldAddress(hash)), address);
    }

    #[test]
    fn shard_id() {
        let shard_id = 0xCAA;
        let world_id = 0xBEE;
        let world_address = WorldAddress::new(shard_id, world_id);
        assert_eq!(shard_id, world_address.shard_id());
    }

    #[test]
    fn world_id() {
        let world_id = 0xCAA;
        let shard_id = 0xBEE;
        let world_address = WorldAddress::new(shard_id, world_id);
        assert_eq!(world_id, world_address.world_id());
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
        let world_address = WorldAddress::from_hash(hash).unwrap();
        assert_eq!(shard_id, world_address.shard_id());
    }

    #[test]
    fn world_id_from_hash() {
        let hash = {
            let mut hash = H256::random();
            hash[0] = PREFIX;
            hash[1] = 0;
            hash
        };
        assert_eq!(::std::mem::size_of::<u16>(), ::std::mem::size_of::<WorldId>());
        let world_id = ((hash[4] as WorldId) << 8) + (hash[5] as WorldId);
        let world_address = WorldAddress::from_hash(hash).unwrap();
        assert_eq!(world_id, world_address.world_id());
    }
}

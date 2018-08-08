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

use ccrypto::BLAKE_NULL_RLP;
use ckey::Address;
use ctypes::ShardId;
use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::cache::CacheableItem;

#[derive(Clone, Debug)]
pub struct Shard {
    root: H256,
    owners: Vec<Address>,
}

impl Shard {
    pub fn new(shard_root: H256, owners: Vec<Address>) -> Self {
        Self {
            root: shard_root,
            owners,
        }
    }

    pub fn root(&self) -> &H256 {
        &self.root
    }

    pub fn set_root(&mut self, root: H256) {
        self.root = root;
    }

    pub fn owners(&self) -> &[Address] {
        debug_assert_ne!(Vec::<Address>::new(), self.owners);
        &self.owners
    }

    pub fn set_owners(&mut self, owners: Vec<Address>) {
        debug_assert_ne!(Vec::<Address>::new(), owners);
        self.owners = owners;
    }
}

impl CacheableItem for Shard {
    type Address = ShardAddress;

    fn is_null(&self) -> bool {
        self.root == BLAKE_NULL_RLP
    }
}

const PREFIX: u8 = super::SHARD_PREFIX;

impl Encodable for Shard {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3).append(&PREFIX).append(&self.root).append_list(&self.owners);
    }
}

impl Decodable for Shard {
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
            root: rlp.val_at(1)?,
            owners: rlp.list_at(2)?,
        })
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ShardAddress(H256);

impl_address!(TOP, ShardAddress, PREFIX);

impl ShardAddress {
    pub fn new(shard_id: ShardId) -> Self {
        Self::from_transaction_hash(H256::from_slice(b"shard"), shard_id.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_shard_id_makes_different_address() {
        let address1 = ShardAddress::new(0);
        let address2 = ShardAddress::new(1);
        assert_ne!(address1, address2);
        assert_eq!(address1[0], PREFIX);
        assert_eq!(address2[0], PREFIX);
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
                break
            }
            hash
        };
        let address = ShardAddress::from_hash(hash);
        assert!(address.is_none());
    }

    #[test]
    fn parse_return_some() {
        let hash = {
            let mut hash = H256::random();
            hash[0] = PREFIX;
            hash
        };
        let address = ShardAddress::from_hash(hash);
        assert_eq!(Some(ShardAddress(hash)), address);
    }
}

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
use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::CacheableItem;

#[derive(Clone, Debug)]
pub struct Shard {
    root: H256,
}

impl Shard {
    pub fn new(shard_root: H256) -> Self {
        Self {
            root: shard_root,
        }
    }

    pub fn root(&self) -> &H256 {
        &self.root
    }

    pub fn set_root(&mut self, root: H256) {
        self.root = root;
    }
}

impl CacheableItem for Shard {
    type Address = ShardAddress;

    fn is_null(&self) -> bool {
        self.root == BLAKE_NULL_RLP
    }
}

const PREFIX: u8 = 'H' as u8;

impl Encodable for Shard {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2).append(&PREFIX).append(&self.root);
    }
}

impl Decodable for Shard {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 2 {
            return Err(DecoderError::RlpInvalidLength)
        }
        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for asset", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            root: rlp.val_at(1)?,
        })
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ShardAddress(H256);

impl_address!(TOP, ShardAddress, PREFIX);

impl ShardAddress {
    pub fn new(shard_id: u32) -> Self {
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
        let address = ShardAddress::from_hash(hash);
        assert!(address.is_none());
    }

    #[test]
    fn parse_return_some() {
        let hash = {
            let mut hash = H256::random();
            hash[0..8].clone_from_slice(&[PREFIX, 0, 0, 0, 0, 0, 0, 0]);
            hash
        };
        let address = ShardAddress::from_hash(hash.clone());
        assert_eq!(Some(ShardAddress(hash)), address);
    }
}

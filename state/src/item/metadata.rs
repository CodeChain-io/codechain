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

use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use ctypes::ShardId;

use crate::CacheableItem;

#[derive(Clone, Debug)]
pub struct Metadata {
    number_of_shards: ShardId,
}

impl Metadata {
    pub fn new(number_of_shards: ShardId) -> Self {
        Self {
            number_of_shards,
        }
    }

    pub fn number_of_shards(&self) -> &ShardId {
        &self.number_of_shards
    }

    pub fn increase_number_of_shards(&mut self) -> ShardId {
        let r = self.number_of_shards;
        self.number_of_shards += 1;
        r
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Self::new(0)
    }
}

impl CacheableItem for Metadata {
    type Address = MetadataAddress;

    fn is_null(&self) -> bool {
        self.number_of_shards == 0
    }
}

const PREFIX: u8 = super::METADATA_PREFIX;

impl Encodable for Metadata {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2).append(&PREFIX).append(&self.number_of_shards);
    }
}

impl Decodable for Metadata {
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
            number_of_shards: rlp.val_at(1)?,
        })
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MetadataAddress(H256);

impl_address!(TOP, MetadataAddress, PREFIX);

impl MetadataAddress {
    pub fn new() -> Self {
        Self::from_transaction_hash(H256::from_slice(b"metadata address"), 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let address = MetadataAddress::from_hash(hash);
        assert!(address.is_none());
    }

    #[test]
    fn parse_return_some() {
        let hash = {
            let mut hash = H256::random();
            hash[0] = PREFIX;
            hash
        };
        let address = MetadataAddress::from_hash(hash);
        assert_eq!(Some(MetadataAddress(hash)), address);
    }
}

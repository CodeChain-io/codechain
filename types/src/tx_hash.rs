// Copyright 2019 Kodebox, Inc.
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


use std::fmt::{self, Display, Formatter};
use std::ops::Deref;

use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};


#[derive(Clone, Copy, Default, Eq, Hash, PartialEq, Debug, Deserialize, Serialize)]
pub struct TxHash(H256);

impl From<H256> for TxHash {
    fn from(h: H256) -> Self {
        Self(h)
    }
}

impl Deref for TxHash {
    type Target = H256;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for TxHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        self.0.fmt(f)
    }
}

impl Encodable for TxHash {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.0.rlp_append(s);
    }
}

impl Decodable for TxHash {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        Ok(H256::decode(rlp)?.into())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use rlp::{self, rlp_encode_and_decode_test};

    use super::*;

    #[test]
    fn hash_of_tx_hash_and_h256_are_the_same() {
        let h256 = H256::random();
        let tx_hash = TxHash(h256);

        let mut hasher_of_h256 = DefaultHasher::new();
        let mut hasher_of_tracker = DefaultHasher::new();

        h256.hash(&mut hasher_of_h256);
        tx_hash.hash(&mut hasher_of_tracker);

        assert_eq!(hasher_of_h256.finish(), hasher_of_tracker.finish());
    }

    #[test]
    fn rlp_of_tx_hash_can_be_decoded_to_h256() {
        let h256 = H256::random();
        let tx_hash = TxHash(h256);

        let encoded = rlp::encode(&tx_hash);
        let decoded = rlp::decode(&*encoded);

        assert_eq!(h256, decoded);
    }

    #[test]
    fn rlp_of_h256_can_be_decoded_to_tx_hash() {
        let h256 = H256::random();

        let encoded = rlp::encode(&h256);
        let decoded = rlp::decode(&*encoded);

        let tx_hash = TxHash(h256);
        assert_eq!(tx_hash, decoded);
    }

    #[test]
    fn rlp() {
        rlp_encode_and_decode_test!(TxHash(H256::random()));
    }
}

// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use ccrypto::Blake;
use ckey::Address;
use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use crate::CacheableItem;

/// Text stored in the DB. Used by Store/Remove Action.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Text {
    // Content of the text
    content: String,
    // Certifier of the text
    certifier: Address,
}

impl Text {
    pub fn new(content: &str, certifier: &Address) -> Self {
        Self {
            content: content.to_string(),
            certifier: *certifier,
        }
    }

    /// Get reference of the content of the text
    pub fn content(&self) -> &String {
        &self.content
    }

    /// Get reference of the certifier of the text
    pub fn certifier(&self) -> &Address {
        &self.certifier
    }

    /// Get blake hash of the content of the text
    pub fn content_hash(&self) -> H256 {
        let rlp = self.content.rlp_bytes();
        Blake::blake(rlp)
    }
}

impl CacheableItem for Text {
    type Address = H256;
    /// Check if content is empty and certifier is null.
    fn is_null(&self) -> bool {
        self.content.is_empty() && self.certifier.is_zero()
    }
}

const PREFIX: u8 = super::TEXT_PREFIX;

impl Encodable for Text {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3);
        s.append(&PREFIX);
        s.append(&self.content);
        s.append(&self.certifier);
    }
}

impl Decodable for Text {
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
            content: rlp.val_at(1)?,
            certifier: rlp.val_at(2)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn rlp_encode_and_decode() {
        rlp_encode_and_decode_test!(Text {
            content: "CodeChain".to_string(),
            certifier: Address::random()
        });
    }

    #[test]
    fn cachable_item_is_null() {
        let text: Text = Default::default();
        assert!(text.is_null());
    }
}

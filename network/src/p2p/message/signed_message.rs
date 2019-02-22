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

use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::Signature;
use crate::session::Session;

#[derive(Debug, PartialEq)]
pub struct SignedMessage {
    pub message: Vec<u8>,
    signature: Signature,
}

impl SignedMessage {
    pub fn new<M>(message: &M, session: &Session) -> Self
    where
        M: Encodable, {
        let message = message.rlp_bytes().into_vec();
        let signature = session.sign(&message);
        Self {
            message,
            signature,
        }
    }

    pub fn is_valid(&self, session: &Session) -> bool {
        session.sign(&self.message) == self.signature
    }
}

impl Encodable for SignedMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2).append(&self.message).append(&self.signature);
    }
}

impl Decodable for SignedMessage {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 2 {
            return Err(DecoderError::Custom("Cannot decode a signed message"))
        }
        Ok(Self {
            message: rlp.val_at(0)?,
            signature: rlp.val_at(1)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn rlp_of_signed_message() {
        let message = vec![];
        let signature = Signature::random();
        let signed = SignedMessage {
            message,
            signature,
        };
        rlp_encode_and_decode_test!(signed);
    }
}

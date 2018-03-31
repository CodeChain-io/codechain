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

use rlp::{UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};

use super::message::Message;
use super::super::super::session::Session;
use super::Signature;

pub struct SignedMessage {
    pub message: Vec<u8>,
    signature: Signature,
}

impl SignedMessage {
    pub fn new(message: Message, session: &Session) -> Option<Self> {
        let message = message.rlp_bytes().into_vec();
        session.sign(&message)
            .map(|signature| {
                Self {
                    message,
                    signature,
                }
            })
    }

    pub fn is_valid(&self, session: &Session) -> bool {
        session.sign(&self.message)
            .map(|signature| signature == self.signature)
            .unwrap_or(false)
    }
}

impl Encodable for SignedMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2)
            .append(&self.message)
            .append(&self.signature);
    }
}

impl Decodable for SignedMessage {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 2 {
            return Err(DecoderError::Custom("invalid message"))
        }
        let message: Vec<u8> = rlp.val_at(0)?;
        let signature: Signature = rlp.val_at(1)?;
        Ok(Self {
            message,
            signature,
        })
    }
}


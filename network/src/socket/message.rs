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

use ckey::Public;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

type Version = u32;
type Raw = Vec<u8>;
type Seq = u64;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub struct Message {
    version: Version,
    seq: Seq,
    body: Body,
}

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum Body {
    SecretRequest(Public),
    SecretAllowed(Public),
    SecretDenied(String),
    NonceRequest(Raw),
    NonceAllowed(Raw),
}

const SECRET_REQUEST: u8 = 0x03;
const SECRET_ALLOWED: u8 = 0x04;
const SECRET_DENIED: u8 = 0x05;

const NONCE_REQUEST: u8 = 0x6;
const NONCE_ALLOWED: u8 = 0x7;

impl Message {
    pub fn secret_request(seq: Seq, key: Public) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::SecretRequest(key),
        }
    }

    pub fn secret_allowed(seq: Seq, key: Public) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::SecretAllowed(key),
        }
    }

    pub fn secret_denied(seq: Seq, reason: String) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::SecretDenied(reason),
        }
    }

    pub fn nonce_request(seq: Seq, body: Vec<u8>) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::NonceRequest(body),
        }
    }

    pub fn nonce_allowed(seq: Seq, body: Vec<u8>) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::NonceAllowed(body),
        }
    }

    pub fn protocol_id(&self) -> u8 {
        match self.body {
            Body::SecretRequest(_) => SECRET_REQUEST,
            Body::SecretAllowed(_) => SECRET_ALLOWED,
            Body::SecretDenied(_) => SECRET_DENIED,
            Body::NonceRequest(_) => NONCE_REQUEST,
            Body::NonceAllowed(_) => NONCE_ALLOWED,
        }
    }

    pub fn body(&self) -> &Body {
        &self.body
    }

    pub fn seq(&self) -> Seq {
        self.seq
    }

    #[allow(dead_code)]
    pub fn version(&self) -> Version {
        self.version
    }

    fn item_count(&self) -> usize {
        4
    }
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        let version = self.version;
        let seq = self.seq;
        s.begin_list(self.item_count()).append(&version).append(&seq).append(&self.protocol_id());
        match &self.body {
            Body::SecretRequest(key) => {
                s.append(key);
            }
            Body::SecretAllowed(key) => {
                s.append(key);
            }
            Body::SecretDenied(reason) => {
                s.append(reason);
            }
            Body::NonceRequest(body) => {
                s.append(body);
            }
            Body::NonceAllowed(body) => {
                s.append(body);
            }
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let version: Version = rlp.val_at(0)?;
        let seq: Seq = rlp.val_at(1)?;
        let protocol_id: u8 = rlp.val_at(2)?;
        debug_assert_eq!(0, version);
        let message = match protocol_id {
            SECRET_REQUEST => {
                let key: Public = rlp.val_at(3)?;
                Message::secret_request(seq, key)
            }
            SECRET_ALLOWED => {
                let key: Public = rlp.val_at(3)?;
                Message::secret_allowed(seq, key)
            }
            SECRET_DENIED => {
                let reason: String = rlp.val_at(3)?;
                Message::secret_denied(seq, reason)
            }
            NONCE_REQUEST => {
                let body: Raw = rlp.val_at(3)?;
                Message::nonce_request(seq, body)
            }
            NONCE_ALLOWED => {
                let body: Raw = rlp.val_at(3)?;
                Message::nonce_allowed(seq, body)
            }
            _ => return Err(DecoderError::Custom("Invalid protocol id")),
        };
        if message.item_count() != rlp.item_count()? {
            return Err(DecoderError::RlpInvalidLength)
        }
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn encode_and_decode_nonce_request() {
        const SEQ: Seq = 0;

        let nonce = 32;
        let nonce = nonce.rlp_bytes();

        let req = Message::nonce_request(SEQ, nonce.into_vec());
        rlp_encode_and_decode_test!(req);
    }

    #[test]
    fn encode_and_decode_nonce_allowed() {
        const SEQ: Seq = 37;

        let nonce = 4;
        let nonce = nonce.rlp_bytes();

        let allowed = Message::nonce_allowed(SEQ, nonce.into_vec());
        rlp_encode_and_decode_test!(allowed);
    }

    #[test]
    fn encode_and_decode_large_nonce_request() {
        let nonce = 0xDEAD_BEEFu128;
        let nonce = nonce.rlp_bytes();

        const SEQ: Seq = 0;

        let req = Message::nonce_request(SEQ, nonce.into_vec());
        rlp_encode_and_decode_test!(req);
    }

    #[test]
    fn encode_and_decode_large_nonce_allowed() {
        let nonce = 0x00CC_AFEC;
        let nonce = nonce.rlp_bytes();

        const SEQ: Seq = 0x4a;

        let allowed = Message::nonce_allowed(SEQ, nonce.into_vec());
        rlp_encode_and_decode_test!(allowed);
    }
}

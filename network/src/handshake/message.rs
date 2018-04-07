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

use ctypes::Public;
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
    ConnectionRequest(Raw),
    ConnectionAllowed(Raw),
    ConnectionDenied(String),
    EcdhRequest(Public),
    EcdhAllowed(Public),
    EcdhDenied(String),
}

const CONNECTION_REQUEST: u8 = 0x1;
const CONNECTION_ALLOWED: u8 = 0x2;
const CONNECTION_DENIED: u8 = 0x3;

const ECDH_REQUEST: u8 = 0x04;
const ECDH_ALLOWED: u8 = 0x05;
const ECDH_DENIED: u8 = 0x06;

impl Message {
    pub fn connection_request(seq: Seq, body: Vec<u8>) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::ConnectionRequest(body),
        }
    }

    pub fn connection_allowed(seq: Seq, body: Vec<u8>) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::ConnectionAllowed(body),
        }
    }

    pub fn connection_denied(seq: Seq, reason: String) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::ConnectionDenied(reason),
        }
    }

    pub fn ecdh_request(seq: Seq, key: Public) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::EcdhRequest(key),
        }
    }

    pub fn ecdh_allowed(seq: Seq, key: Public) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::EcdhAllowed(key),
        }
    }

    pub fn ecdh_denied(seq: Seq, reason: String) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::EcdhDenied(reason),
        }
    }

    pub fn protocol_id(&self) -> u8 {
        match self.body {
            Body::ConnectionRequest(_) => CONNECTION_REQUEST,
            Body::ConnectionAllowed(_) => CONNECTION_ALLOWED,
            Body::ConnectionDenied(_) => CONNECTION_DENIED,
            Body::EcdhRequest(_) => ECDH_REQUEST,
            Body::EcdhAllowed(_) => ECDH_ALLOWED,
            Body::EcdhDenied(_) => ECDH_DENIED,
        }
    }

    pub fn body(&self) -> &Body {
        &self.body
    }

    pub fn seq(&self) -> Seq {
        self.seq
    }

    pub fn version(&self) -> Version {
        self.version
    }
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        let version = self.version;
        let seq = self.seq;
        s.begin_list(4)
            .append(&version)
            .append(&seq)
            .append(&self.protocol_id());
        match self.body {
            Body::ConnectionRequest(ref body) => {
                s.append(body);
            }
            Body::ConnectionAllowed(ref body) => {
                s.append(body);
            }
            Body::ConnectionDenied(ref reason) => {
                s.append(reason);
            }
            Body::EcdhRequest(ref key) => {
                s.append(key);
            }
            Body::EcdhAllowed(ref key) => {
                s.append(key);
            }
            Body::EcdhDenied(ref reason) => {
                s.append(reason);
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
        match protocol_id {
            CONNECTION_REQUEST => {
                let body: Raw = rlp.val_at(3)?;
                Ok(Message::connection_request(seq, body))
            }
            CONNECTION_ALLOWED => {
                let body: Raw = rlp.val_at(3)?;
                Ok(Message::connection_allowed(seq, body))
            }
            CONNECTION_DENIED => {
                let reason: String = rlp.val_at(3)?;
                Ok(Message::connection_denied(seq, reason))
            }
            ECDH_REQUEST => {
                let key: Public = rlp.val_at(3)?;
                Ok(Message::ecdh_request(seq, key))
            }
            ECDH_ALLOWED => {
                let key: Public = rlp.val_at(3)?;
                Ok(Message::ecdh_allowed(seq, key))
            }
            ECDH_DENIED => {
                let reason: String = rlp.val_at(3)?;
                Ok(Message::ecdh_denied(seq, reason))
            }
            _ => Err(DecoderError::Custom("Invalid protocol id")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rlp::{Decodable, Encodable, UntrustedRlp};

    use super::Message;
    use super::Seq;
    use super::Version;
    use super::super::super::session::Nonce;

    const SINGLE: u8 = 0x80;
    const LIST: u8 = 0xc0;
    use super::CONNECTION_REQUEST;
    use super::CONNECTION_ALLOWED;
    use super::CONNECTION_DENIED;

    const VERSION: Version = 0;

    #[test]
    fn request_rlp_encode() {
        const SEQ: Seq = 0;

        const NONCE: u8 = 32;
        let nonce = NONCE.rlp_bytes();

        let req = Message::connection_request(SEQ, nonce.clone().into_vec());
        let bytes = req.rlp_bytes();
        assert_eq!(
            1
                   + 1 /* version */ + 1 /* seq */
                   + 1 /* protocol id */
                   + nonce.len(), /* rlp(nonce) */
            bytes.len()
        );

        // length prefix
        assert_eq!(LIST as usize + bytes.len() - 1, bytes[0] as usize);

        // version
        assert_eq!(SINGLE as Version + VERSION, bytes[1] as Version);

        // seq
        assert_eq!(SINGLE as Seq + SEQ, bytes[2] as Seq);

        // protocol id
        assert_eq!(CONNECTION_REQUEST, bytes[3]);

        // nonce
        const START_OF_NONCE: usize = 4;
        assert_eq!(nonce.into_vec().as_slice(), &bytes[START_OF_NONCE..]);
    }

    #[test]
    fn allowed_rlp_encode() {
        const SEQ: Seq = 37;

        const NONCE: Nonce = 4;
        let nonce = NONCE.rlp_bytes();

        let allowed = Message::connection_allowed(SEQ, nonce.clone().into_vec());

        let bytes = allowed.rlp_bytes();
        assert_eq!(
            1 + 1 /* version */ + 1 /* seq */
                       + 1 /* protocol id */
                       + nonce.len(), /* rlp(nonce) */
            bytes.len()
        );

        // length prefix
        assert_eq!(LIST as usize + bytes.len() - 1, bytes[0] as usize);

        // version
        assert_eq!(SINGLE as Version + VERSION, bytes[1] as Version);

        // seq
        assert_eq!(SEQ, bytes[2] as Seq);

        // protocol id
        assert_eq!(CONNECTION_ALLOWED, bytes[3]);

        // nonce
        const START_OF_NONCE: usize = 4;
        assert_eq!(nonce.into_vec().as_slice(), &bytes[START_OF_NONCE..]);
    }

    #[test]
    fn denied_rlp_encode() {
        const SEQ: Seq = 6;

        const REASON: &str = "connection denied";
        let reason_len: usize = REASON.len();

        let denied = Message::connection_denied(SEQ, REASON.to_string());

        let bytes = denied.rlp_bytes();
        assert_eq!(
            1
                       + 1 /* version */ + 1 /* seq */
                       + 1 /* protocol id */
                       + 1 + reason_len, /* reason */
            bytes.len()
        );

        // length prefix
        assert_eq!(LIST as usize + bytes.len() - 1, bytes[0] as usize);

        // version
        assert_eq!(SINGLE as Version + VERSION, bytes[1] as Version);

        // seq
        assert_eq!(SEQ, bytes[2] as Seq);

        // protocol id
        assert_eq!(CONNECTION_DENIED, bytes[3]);
        const START_OF_REASON: usize = 4;

        // reason
        assert_eq!(SINGLE + reason_len as u8, bytes[START_OF_REASON]);
        assert_eq!(
            REASON.as_bytes(),
            &bytes[(START_OF_REASON + 1)..(START_OF_REASON + 1 + reason_len)]
        );
    }

    #[test]
    fn request_rlp_decode() {
        const NONCE: Nonce = 42;
        const SEQ: Seq = 17;
        let nonce = NONCE.rlp_bytes().into_vec();

        let mut bytes: Vec<u8> = vec![
            LIST + 1 /* version */ + 1 /* seq */
                + 1 /* protocol id */
                + nonce.len() as u8, /* rlp(nonce) */
        ];

        bytes.push(SINGLE + VERSION as u8);

        bytes.push(SEQ as u8);

        bytes.push(CONNECTION_REQUEST);

        bytes.extend_from_slice(nonce.as_slice());

        assert_eq!(
            1 + 1 /* version */ + 1 /* seq */
                + 1 /* protocol id */ + nonce.len(), /* rlp(nonce) */
            bytes.len()
        );

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_request(SEQ, nonce), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn allowed_rlp_decode() {
        const NONCE: Nonce = 37;
        const SEQ: Seq = 62;
        let nonce = NONCE.rlp_bytes().into_vec();

        let mut bytes: Vec<u8> = vec![
            LIST + 1 /* version */ + 1 /* seq */
                + 1 /* protocol id */
                + nonce.len() as u8, /* rlp(nonce) */
        ];

        bytes.push(SINGLE + VERSION as u8);

        bytes.push(SEQ as u8);

        bytes.push(CONNECTION_ALLOWED);

        bytes.extend_from_slice(nonce.as_slice());

        assert_eq!(
            1 + 1 /* version */ + 1 /* seq */
                       + 1 /* protocol id */
                       + nonce.len(), /* rlp(nonce) */
            bytes.len()
        );

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_allowed(SEQ, nonce), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn denied_rlp_decode() {
        const REASON: &str = "decode connection denied";
        let reason_len: usize = REASON.len();

        const SEQ: Seq = 62;

        let mut bytes: Vec<u8> = vec![
            LIST + 1 /* version */ + 1 /* seq */
                + 1 /* protocol id */
                + 1 + reason_len as u8, /* reason */
        ];

        bytes.push(SINGLE + VERSION as u8);

        bytes.push(SEQ as u8);

        bytes.push(CONNECTION_DENIED);

        bytes.extend_from_slice(REASON.rlp_bytes().into_vec().as_slice());

        assert_eq!(
            1 + 1 /* version */ + 1 /* seq */
                + 1 /* protocol id */
                + 1 + reason_len,
            bytes.len()
        );

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_denied(SEQ, REASON.to_string()), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn request_rlp_encode_with_large_nonce() {
        const NONCE: Nonce = 0xDEADBEEF;
        let nonce = NONCE.rlp_bytes();

        const SEQ: Seq = 0;

        let req = Message::connection_request(SEQ, nonce.clone().into_vec());
        let bytes = req.rlp_bytes();
        assert_eq!(
            1
                   + 1 /* version */ + 1 /* seq */
                   + 1 /* protocol id */
                   + 1 + nonce.len(), /* rlp(nonce) */
            bytes.len()
        );

        // length prefix
        assert_eq!(LIST as usize + bytes.len() - 1, bytes[0] as usize);

        // version
        assert_eq!(SINGLE as Version + VERSION, bytes[1] as Version);

        // seq
        assert_eq!(SINGLE as Seq + SEQ, bytes[2] as Seq);

        // protocol id
        assert_eq!(CONNECTION_REQUEST, bytes[3]);

        // nonce
        const START_OF_NONCE: usize = 4;
        assert_eq!(SINGLE + nonce.len() as u8, bytes[START_OF_NONCE]);
        assert_eq!(nonce.into_vec().as_slice(), &bytes[(START_OF_NONCE + 1)..]);
    }

    #[test]
    fn allowed_encode_with_large_nonce() {
        const NONCE: Nonce = 0xCCAFEC;
        let nonce = NONCE.rlp_bytes();

        const SEQ: Seq = 0x4a;

        let allowed = Message::connection_allowed(SEQ, nonce.clone().into_vec());
        let bytes = allowed.rlp_bytes();
        assert_eq!(
            1
                   +1 /* version */ + 1 /* seq */
                   + 1 /* protocol id */
                   + 1 + nonce.len(), /* rlp(nonce) */
            bytes.len()
        );

        // length prefix
        assert_eq!(LIST as usize + bytes.len() - 1, bytes[0] as usize);

        // version
        assert_eq!(SINGLE as Version + VERSION, bytes[1] as Version);

        // seq
        assert_eq!(SEQ, bytes[2] as Seq);

        // name
        assert_eq!(CONNECTION_ALLOWED, bytes[3]);

        // nonce
        const START_OF_NONCE: usize = 4;
        assert_eq!(SINGLE + nonce.len() as u8, bytes[START_OF_NONCE]);
        assert_eq!(nonce.into_vec().as_slice(), &bytes[(1 + START_OF_NONCE)..]);
    }

    #[test]
    fn request_rlp_decode_with_large_nonce() {
        const NONCE: Nonce = 0xDEADCAFE;
        const NONCE_LEN: u8 = 4;
        let nonce = NONCE.rlp_bytes().into_vec();

        const SEQ: Seq = 0x39;
        let mut bytes: Vec<u8> = vec![
            LIST + 1 /* version */ + 1 /* seq */
                + 1 /* protocol id */
                + 1 + nonce.len() as u8, /* rlp(nonce) */
        ];

        bytes.push(SINGLE + VERSION as u8);

        bytes.push(SEQ as u8);

        bytes.push(CONNECTION_REQUEST);

        bytes.push(SINGLE + nonce.len() as u8);
        bytes.extend_from_slice(nonce.as_slice());

        assert_eq!(
            1 + 1 /* version */ + 1 /* seq */
                + 1 /* protocol id */
                + 1 + nonce.len(), /* rlp(nonce) */
            bytes.len()
        );

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_request(SEQ, nonce), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn allowed_rlp_decode_with_large_nonce() {
        const NONCE: Nonce = 0xCCCAFE;
        let nonce = NONCE.rlp_bytes().into_vec();

        const SEQ: Seq = 0x21;

        let mut bytes: Vec<u8> = vec![
            LIST + 1 /* version */ + 1 /* seq */
                + 1 /* protocol id */
                + 1 + nonce.len() as u8, /* rlp(nonce) */
        ];

        bytes.push(SINGLE + VERSION as u8);

        bytes.push(SEQ as u8);

        bytes.push(CONNECTION_ALLOWED);

        bytes.push(SINGLE + nonce.len() as u8);
        bytes.extend_from_slice(nonce.as_slice());

        assert_eq!(
            1 + 1 /* version */ + 1 /* seq */
                + 1 /* protocol id */
                + 1 + nonce.len(), /* rlp(nonce) */
            bytes.len()
        );

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_allowed(SEQ, nonce), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn encode_and_decode_connection_request() {
        let nonce: Nonce = 0xCAFE;
        let nonce = nonce.rlp_bytes().into_vec();
        let msg = Message::connection_request(0, nonce);
        let encoded = msg.rlp_bytes();
        let rlp = UntrustedRlp::new(&encoded);
        let decoded = Decodable::decode(&rlp).unwrap();

        assert_eq!(msg, decoded);
    }
}

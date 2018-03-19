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

use super::super::session::Nonce;

type Version = u32;
type Name = &'static str;
type Raw = Vec<u8>;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum Message {
    ConnectionRequest(Version, Raw),
    ConnectionAllowed(Version, Raw),
    ConnectionDenied(Version, String),
}

const REQUEST_LEN: u8 = 18;
const REQUEST: &str = "connection-request";
const ALLOWED_LEN: u8 = 18;
const ALLOWED: &str = "connection-allowed";
const DENIED_LEN: u8 = 17;
const DENIED: &str = "connection-denied";

impl Message {
    pub fn connection_request(body: Vec<u8>) -> Self {
        Message::ConnectionRequest(0, body)
    }

    pub fn connection_allowed(body: Vec<u8>) -> Self {
        Message::ConnectionAllowed(0, body)
    }

    pub fn connection_denied(reason: String) -> Self {
        Message::ConnectionDenied(0, reason)
    }

    pub fn name(&self) -> &'static str {
        match self {
            &Message::ConnectionRequest(_, _) => REQUEST,
            &Message::ConnectionAllowed(_, _) => ALLOWED,
            &Message::ConnectionDenied(_, _) => DENIED,
        }
    }
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            &Message::ConnectionRequest(version, ref body) => {
                s.begin_list(3)
                    .append(&self.name())
                    .append(&version)
                    .append(body);
            },
            &Message::ConnectionAllowed(version, ref body) => {
                s.begin_list(3)
                    .append(&self.name())
                    .append(&version)
                    .append(body);
            },
            &Message::ConnectionDenied(version, ref reason) => {
                s.begin_list(3)
                    .append(&self.name())
                    .append(&version)
                    .append(reason);
            },
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let name: String = rlp.val_at(0)?;
        let version: Version = rlp.val_at(1)?;
        debug_assert_eq!(0, version);
        match name.as_ref() {
            REQUEST => {
                let body: Raw = rlp.val_at(2)?;
                Ok(Message::connection_request(body))
            },
            ALLOWED => {
                let body: Raw = rlp.val_at(2)?;
                Ok(Message::connection_allowed(body))
            },
            DENIED => {
                let reason: String = rlp.val_at(2)?;
                Ok(Message::connection_denied(reason))
            },
            _ =>
                Err(DecoderError::Custom("Invalid message name")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rlp::{ Decodable, Encodable, UntrustedRlp };

    use super::Message;
    use super::Nonce;

    const SINGLE: u8 = 0x80;
    const LIST: u8 = 0xc0;
    use super::REQUEST_LEN;
    use super::REQUEST;
    use super::ALLOWED_LEN;
    use super::ALLOWED;
    use super::DENIED_LEN;
    use super::DENIED;

    #[test]
    fn request_rlp_encode() {
        let nonce = vec![32];

        let req = Message::connection_request(nonce);
        let bytes = req.rlp_bytes();

        // prefix
        assert_eq!(1 + 1 + REQUEST_LEN as usize + 1 + 1, bytes.len());
        assert_eq!(LIST + 1 + REQUEST_LEN + 1 + 1, bytes[0]);

        // name
        assert_eq!(SINGLE + REQUEST_LEN, bytes[1]);
        const START_OF_TYPE: usize = 2;
        const INDEX_OF_VERSION: usize = START_OF_TYPE + REQUEST_LEN as usize;
        assert_eq!(REQUEST.as_bytes(), &bytes[START_OF_TYPE..INDEX_OF_VERSION]);

        // version
        assert_eq!(SINGLE, bytes[INDEX_OF_VERSION]);

        const START_OF_NONCE: usize = INDEX_OF_VERSION + 1;
        // nonce
        assert_eq!(32, bytes[START_OF_NONCE]);
    }

    #[test]
    fn allowed_rlp_encode() {
        const NONCE: Nonce = 4;
        let nonce = NONCE.rlp_bytes();

        let allowed = Message::connection_allowed(nonce.into_vec());
        let bytes = allowed.rlp_bytes();
        assert_eq!(1 + 1 + ALLOWED_LEN as usize + 1 + 1, bytes.len());
        assert_eq!(LIST + 1 + ALLOWED_LEN + 1 + 1, bytes[0]);
        assert_eq!(SINGLE + ALLOWED_LEN, bytes[1]);

        const START_OF_TYPE: usize = 2;
        const INDEX_OF_VERSION: usize = START_OF_TYPE + ALLOWED_LEN as usize;

        assert_eq!(ALLOWED.as_bytes(), &bytes[START_OF_TYPE..INDEX_OF_VERSION]);

        assert_eq!(SINGLE + 0, bytes[INDEX_OF_VERSION]);

        const START_OF_NONCE: usize = INDEX_OF_VERSION + 1;
        assert_eq!(4, bytes[START_OF_NONCE]);
    }

    #[test]
    fn denied_rlp_encode() {
        const REASON: &str = "connection denied";
        let reason_len: usize = REASON.len();

        let denied = Message::connection_denied(REASON.to_string());
        let bytes = denied.rlp_bytes();
        assert_eq!(1 + 1 + 1 + DENIED_LEN as usize + 1 + reason_len, bytes.len());
        assert_eq!(LIST + 1 + 1 + DENIED_LEN + 1 + reason_len as u8, bytes[0]);
        assert_eq!(SINGLE + DENIED_LEN, bytes[1]);

        const START_OF_TYPE: usize = 2;
        const INDEX_OF_VERSION: usize = START_OF_TYPE + DENIED_LEN as usize;

        assert_eq!(DENIED.as_bytes(), &bytes[START_OF_TYPE..INDEX_OF_VERSION]);

        const VERSION: u8 = 0;
        assert_eq!(SINGLE + VERSION, bytes[INDEX_OF_VERSION]);

        const START_OF_REASON: usize = INDEX_OF_VERSION + 1;
        assert_eq!(SINGLE + reason_len as u8, bytes[START_OF_REASON]);
        assert_eq!(REASON.as_bytes(), &bytes[(START_OF_REASON + 1)..(START_OF_REASON + 1 + reason_len)]);
    }

    #[test]
    fn request_rlp_decode() {
        const NONCE: Nonce = 42;
        let mut bytes: Vec<u8> = vec![LIST + 8, SINGLE + REQUEST_LEN];
        bytes.extend_from_slice(REQUEST.as_bytes());
        const VERSION: u8 = 0;
        bytes.push(SINGLE + VERSION);
        bytes.push(NONCE as u8);
        assert_eq!(1 + 1 + REQUEST_LEN as usize + 1 + 1, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        let nonce = vec![42];
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_request(nonce), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn allowed_rlp_decode() {
        const NONCE: Nonce = 42;
        let mut bytes: Vec<u8> = vec![LIST + 1 + ALLOWED_LEN + 1 + 1, SINGLE + ALLOWED_LEN];
        bytes.extend_from_slice(ALLOWED.as_bytes());
        const VERSION: u8 = 0;
        bytes.push(SINGLE + VERSION);
        bytes.push(NONCE as u8);
        assert_eq!(1 + 1 + ALLOWED_LEN as usize + 1 + 1, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        let nonce = vec![42];
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_allowed(nonce), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn denied_rlp_decode() {
        const REASON: &str = "decode connection denied";
        let reason_len: usize = REASON.len();

        let mut bytes: Vec<u8> = vec![LIST + 1 + DENIED_LEN + 1 + 1, SINGLE + DENIED_LEN];
        bytes.extend_from_slice(DENIED.as_bytes());

        const VERSION: u8 = 0;
        bytes.push(SINGLE +VERSION);

        bytes.push(SINGLE + reason_len as u8);

        bytes.extend_from_slice(REASON.as_bytes());
        assert_eq!(1 + 1 + DENIED_LEN as usize + 1 + 1 + reason_len, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_denied(REASON.to_string()), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn request_rlp_encode_with_large_nonce() {
        const NONCE: Nonce = 0xDEADBEEF;
        let nonce = NONCE.rlp_bytes();

        let request = Message::connection_request(nonce.into_vec());
        let bytes = request.rlp_bytes();
        assert_eq!(1 + 1 + REQUEST_LEN as usize + 1 + 1 + 1 + 4, bytes.len());

        assert_eq!(LIST + 1 + REQUEST_LEN + 1 + 1 + 1 + 4, bytes[0]);
        assert_eq!(SINGLE + REQUEST_LEN, bytes[1]);

        const START_OF_TYPE: usize = 2;
        const INDEX_OF_VERSION: usize = START_OF_TYPE + REQUEST_LEN as usize;

        assert_eq!(REQUEST.as_bytes(), &bytes[START_OF_TYPE..INDEX_OF_VERSION]);

        const VERSION: u8 = 0;
        assert_eq!(SINGLE + VERSION, bytes[INDEX_OF_VERSION]);

        const START_OF_NONCE: usize = INDEX_OF_VERSION + 1;

        assert_eq!(SINGLE + 5, bytes[START_OF_NONCE]);
        assert_eq!(SINGLE + 4, bytes[START_OF_NONCE + 1]);
        assert_eq!(0xDE, bytes[START_OF_NONCE + 2]);
        assert_eq!(0xAD, bytes[START_OF_NONCE + 3]);
        assert_eq!(0xBE, bytes[START_OF_NONCE + 4]);
        assert_eq!(0xEF, bytes[START_OF_NONCE + 5]);
    }

    #[test]
    fn allowed_encode_with_large_nonce() {
        const NONCE: Nonce = 0xCCAFEC;
        let nonce = NONCE.rlp_bytes();

        let allowed = Message::connection_allowed(nonce.into_vec());
        let bytes = allowed.rlp_bytes();
        assert_eq!(1 + 1 + ALLOWED_LEN as usize + 1 + 1 + 1 + 3, bytes.len());

        assert_eq!(LIST + 1 + ALLOWED_LEN + 1 + 1 + 1 + 3, bytes[0]);
        assert_eq!(SINGLE + ALLOWED_LEN, bytes[1]);

        const START_OF_TYPE: usize = 2;
        const INDEX_OF_VERSION: usize = START_OF_TYPE + ALLOWED_LEN as usize;

        assert_eq!(ALLOWED.as_bytes(), &bytes[START_OF_TYPE..INDEX_OF_VERSION]);

        assert_eq!(SINGLE, bytes[INDEX_OF_VERSION]);

        const START_OF_NONCE: usize = INDEX_OF_VERSION + 1;

        assert_eq!(SINGLE + 4, bytes[START_OF_NONCE]);
        assert_eq!(SINGLE + 3, bytes[START_OF_NONCE + 1]);
        assert_eq!(0xCC, bytes[START_OF_NONCE + 2]);
        assert_eq!(0xAF, bytes[START_OF_NONCE + 3]);
        assert_eq!(0xEC, bytes[START_OF_NONCE + 4]);
    }

    #[test]
    fn request_rlp_decode_with_large_nonce() {
        const NONCE: Nonce = 0xDEADCAFE;
        const NONCE_LEN: u8 = 4;

        let mut bytes: Vec<u8> = vec![
            LIST + 1 + REQUEST_LEN + 1 + 1 + NONCE_LEN
            , SINGLE + REQUEST_LEN];
        bytes.extend_from_slice(REQUEST.as_bytes());

        const VERSION: u8 = 0;
        bytes.append(&mut vec![SINGLE + VERSION, SINGLE + NONCE_LEN, 0xDE, 0xAD, 0xCA, 0xFE]);

        assert_eq!(1 + 1 + REQUEST_LEN as usize + 1 + 1 + NONCE_LEN as usize, bytes.len());


        let nonce = vec![0xDE, 0xAD, 0xCA, 0xFE];
        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_request(nonce), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn allowed_rlp_decode_with_large_nonce() {
        const NONCE: Nonce = 0xCCCAFE;
        const NONCE_LEN: u8 = 3;

        let mut bytes: Vec<u8> = vec![
            LIST + 1 + ALLOWED_LEN + 1 + 1 + NONCE_LEN
            , SINGLE + ALLOWED_LEN];
        bytes.extend_from_slice(ALLOWED.as_bytes());

        const VERSION: u8 = 0;
        bytes.append(&mut vec![SINGLE + VERSION, SINGLE + NONCE_LEN, 0xCC, 0xCA, 0xFE]);

        assert_eq!(1 + 1 + ALLOWED_LEN as usize + 1 + 1 + NONCE_LEN as usize, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        let nonce = vec![0xCC, 0xCA, 0xFE];
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_allowed(nonce), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }
}

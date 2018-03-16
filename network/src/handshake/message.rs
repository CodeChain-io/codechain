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

use super::handshake::Nonce;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum Message {
    ConnectionRequest(Nonce),
    ConnectionAllowed(Nonce),
    ConnectionDenied(String),
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            &Message::ConnectionRequest(nonce) => {
                s.begin_list(2)
                    .append(&"request")
                    .append(&nonce)
            },
            &Message::ConnectionAllowed(nonce) => {
                s.begin_list(2)
                    .append(&"allowed")
                    .append(&nonce)
            },
            &Message::ConnectionDenied(ref reason) => {
                s.begin_list(2)
                    .append(&"denied")
                    .append(reason)
            },
        };
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let message: String = rlp.val_at(0)?;
        match message.as_ref() {
            "request" => {
                let nonce: Nonce = rlp.val_at(1)?;
                Ok(Message::ConnectionRequest(nonce))
            },
            "allowed" => {
                let nonce: Nonce = rlp.val_at(1)?;
                Ok(Message::ConnectionAllowed(nonce))
            },
            "denied" => {
                let reason: String = rlp.val_at(1)?;
                Ok(Message::ConnectionDenied(reason))
            },
            _ =>
                Err(DecoderError::Custom("Invalid type")),
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
    const REQUEST_LEN: u8 = 7;
    const REQUEST: &[u8; REQUEST_LEN as usize] = b"request";
    const ALLOWED_LEN: u8 = 7;
    const ALLOWED: &[u8; ALLOWED_LEN as usize] = b"allowed";
    const DENIED_LEN: u8 = 6;
    const DENIED: &[u8; DENIED_LEN as usize] = b"denied";

    #[test]
    fn request_rlp_encode() {
        const NONCE: Nonce = 32;
        let req = Message::ConnectionRequest(NONCE);
        let bytes = req.rlp_bytes();
        assert_eq!(1 + REQUEST_LEN as usize + 2, bytes.len());
        assert_eq!(LIST + REQUEST_LEN + 2, bytes[0]);

        assert_eq!(SINGLE + REQUEST_LEN, bytes[1]);

        const START_OF_TYPE: usize = 2;
        const START_OF_NONCE: usize = START_OF_TYPE + REQUEST_LEN as usize;

        assert_eq!(REQUEST, &bytes[START_OF_TYPE..START_OF_NONCE]);

        assert_eq!(NONCE as u8, bytes[START_OF_NONCE]);
    }

    #[test]
    fn allowed_rlp_encode() {
        const NONCE: Nonce = 4;
        let allowed = Message::ConnectionAllowed(NONCE);
        let bytes = allowed.rlp_bytes();
        assert_eq!(1 + 1 + ALLOWED_LEN as usize + 1, bytes.len());
        assert_eq!(LIST + ALLOWED_LEN + 2, bytes[0]);
        assert_eq!(SINGLE + ALLOWED_LEN, bytes[1]);

        const START_OF_TYPE: usize = 2;
        const START_OF_NONCE: usize = START_OF_TYPE + ALLOWED_LEN as usize;

        assert_eq!(ALLOWED, &bytes[START_OF_TYPE..START_OF_NONCE]);
        assert_eq!(NONCE as u8, bytes[START_OF_NONCE]);
    }

    #[test]
    fn denied_rlp_encode() {
        const REASON: &str = "connection denied";
        let reason_len: usize = REASON.len();

        let denied = Message::ConnectionDenied(REASON.to_string());
        let bytes = denied.rlp_bytes();
        assert_eq!(1 + 1 + DENIED_LEN as usize + 1 + reason_len, bytes.len());
        assert_eq!(LIST + 1 + DENIED_LEN + 1 + reason_len as u8, bytes[0]);
        assert_eq!(SINGLE + DENIED_LEN, bytes[1]);

        const START_OF_TYPE: usize = 2;
        const START_OF_REASON: usize = START_OF_TYPE + DENIED_LEN as usize;

        assert_eq!(DENIED, &bytes[START_OF_TYPE..START_OF_REASON]);
        assert_eq!(SINGLE + reason_len as u8, bytes[START_OF_REASON]);
        assert_eq!(REASON.as_bytes(), &bytes[(START_OF_REASON + 1)..(START_OF_REASON + 1 + reason_len)]);
    }

    #[test]
    fn request_rlp_decode() {
        const NONCE: Nonce = 42;
        let mut bytes: Vec<u8> = vec![LIST + 8, SINGLE + REQUEST_LEN];
        bytes.extend_from_slice(REQUEST);
        bytes.push(NONCE as u8);
        assert_eq!(1 + 1 + REQUEST_LEN as usize + 1, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::ConnectionRequest(NONCE), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn allowed_rlp_decode() {
        const NONCE: Nonce = 42;
        let mut bytes: Vec<u8> = vec![LIST + 1 + ALLOWED_LEN + 1, SINGLE + ALLOWED_LEN];
        bytes.extend_from_slice(ALLOWED);
        bytes.push(NONCE as u8);
        assert_eq!(1 + 1 + ALLOWED_LEN as usize + 1, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::ConnectionAllowed(NONCE), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn denied_rlp_decode() {
        const REASON: &str = "decode connection denied";
        let reason_len: usize = REASON.len();

        let mut bytes: Vec<u8> = vec![LIST + 1 + DENIED_LEN + 1, SINGLE + DENIED_LEN];
        bytes.extend_from_slice(DENIED);
        bytes.push(SINGLE + reason_len as u8);
        bytes.extend_from_slice(REASON.as_bytes());
        assert_eq!(1 + 1 + DENIED_LEN as usize + 1 + reason_len, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::ConnectionDenied(REASON.to_string()), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn request_rlp_encode_with_large_nonce() {
        const NONCE: Nonce = 0xDEADBEEF;
        let request = Message::ConnectionRequest(NONCE);
        let bytes = request.rlp_bytes();
        assert_eq!(1 + 1 + REQUEST_LEN as usize + 1 + 4, bytes.len());

        assert_eq!(LIST + 13, bytes[0]);
        assert_eq!(SINGLE + REQUEST_LEN, bytes[1]);

        const START_OF_TYPE: usize = 2;
        const START_OF_NONCE: usize = START_OF_TYPE + REQUEST_LEN as usize;

        assert_eq!(REQUEST, &bytes[START_OF_TYPE..START_OF_NONCE]);

        assert_eq!(SINGLE + 4, bytes[START_OF_NONCE]);
        assert_eq!(0xDE, bytes[START_OF_NONCE + 1]);
        assert_eq!(0xAD, bytes[START_OF_NONCE + 2]);
        assert_eq!(0xBE, bytes[START_OF_NONCE + 3]);
        assert_eq!(0xEF, bytes[START_OF_NONCE + 4]);
    }

    #[test]
    fn allowed_encode_with_large_nonce() {
        const NONCE: Nonce = 0xCCAFEC;
        let allowed = Message::ConnectionAllowed(NONCE);
        let bytes = allowed.rlp_bytes();
        assert_eq!(1 + 1 + ALLOWED_LEN as usize + 1 + 3, bytes.len());

        assert_eq!(LIST + 1 + ALLOWED_LEN + 1 + 3, bytes[0]);
        assert_eq!(SINGLE + ALLOWED_LEN, bytes[1]);

        const START_OF_TYPE: usize = 2;
        const START_OF_NONCE: usize = START_OF_TYPE + ALLOWED_LEN as usize;

        assert_eq!(ALLOWED, &bytes[START_OF_TYPE..START_OF_NONCE]);

        assert_eq!(SINGLE + 3, bytes[START_OF_NONCE]);
        assert_eq!(0xCC, bytes[START_OF_NONCE + 1]);
        assert_eq!(0xAF, bytes[START_OF_NONCE + 2]);
        assert_eq!(0xEC, bytes[START_OF_NONCE + 3]);
    }

    #[test]
    fn request_rlp_decode_with_large_nonce() {
        const NONCE: Nonce = 0xDEADCAFE;
        const NONCE_LEN: u8 = 4;

        let mut bytes: Vec<u8> = vec![
            LIST + 1 + REQUEST_LEN + 1 + NONCE_LEN
            , SINGLE + REQUEST_LEN];
        bytes.extend_from_slice(REQUEST);
        bytes.append(&mut vec![SINGLE + NONCE_LEN, 0xDE, 0xAD, 0xCA, 0xFE]);

        assert_eq!(1 + 1 + REQUEST_LEN as usize + 1 + NONCE_LEN as usize, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::ConnectionRequest(NONCE), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn allowed_rlp_decode_with_large_nonce() {
        const NONCE: Nonce = 0xCCCAFE;
        const NONCE_LEN: u8 = 3;

        let mut bytes: Vec<u8> = vec![
            LIST + 1 + ALLOWED_LEN + 1 + NONCE_LEN
            , SINGLE + ALLOWED_LEN];
        bytes.extend_from_slice(ALLOWED);
        bytes.append(&mut vec![SINGLE + NONCE_LEN, 0xCC, 0xCA, 0xFE]);

        assert_eq!(1 + 1 + ALLOWED_LEN as usize + 1 + NONCE_LEN as usize, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::ConnectionAllowed(NONCE), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }
}

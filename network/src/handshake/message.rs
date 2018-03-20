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
type Raw = Vec<u8>;
type Seq = u64;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum Message {
    ConnectionRequest(Version, Seq, Raw),
    ConnectionAllowed(Version, Seq, Raw),
    ConnectionDenied(Version, Seq, String),
}

const REQUEST_LEN: u8 = 18;
const REQUEST: &str = "connection-request";
const ALLOWED_LEN: u8 = 18;
const ALLOWED: &str = "connection-allowed";
const DENIED_LEN: u8 = 17;
const DENIED: &str = "connection-denied";

impl Message {
    pub fn connection_request(seq: Seq, body: Vec<u8>) -> Self {
        Message::ConnectionRequest(0, seq,body)
    }

    pub fn connection_allowed(seq: Seq, body: Vec<u8>) -> Self {
        Message::ConnectionAllowed(0, seq, body)
    }

    pub fn connection_denied(seq: Seq, reason: String) -> Self {
        Message::ConnectionDenied(0, seq,reason)
    }

    pub fn name(&self) -> &'static str {
        match self {
            &Message::ConnectionRequest(_, _, _) => REQUEST,
            &Message::ConnectionAllowed(_, _, _) => ALLOWED,
            &Message::ConnectionDenied(_, _, _) => DENIED,
        }
    }
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            &Message::ConnectionRequest(version, seq, ref body) => {
                s.begin_list(4)
                    .append(&version)
                    .append(&seq)
                    .append(&self.name())
                    .append(body);
            },
            &Message::ConnectionAllowed(version, seq, ref body) => {
                s.begin_list(4)
                    .append(&version)
                    .append(&seq)
                    .append(&self.name())
                    .append(body);
            },
            &Message::ConnectionDenied(version, seq, ref reason) => {
                s.begin_list(4)
                    .append(&version)
                    .append(&seq)
                    .append(&self.name())
                    .append(reason);
            },
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let version: Version = rlp.val_at(0)?;
        let seq: Seq = rlp.val_at(1)?;
        let name: String = rlp.val_at(2)?;
        debug_assert_eq!(0, version);
        match name.as_ref() {
            REQUEST => {
                let body: Raw = rlp.val_at(3)?;
                Ok(Message::connection_request(seq, body))
            },
            ALLOWED => {
                let body: Raw = rlp.val_at(3)?;
                Ok(Message::connection_allowed(seq, body))
            },
            DENIED => {
                let reason: String = rlp.val_at(3)?;
                Ok(Message::connection_denied(seq, reason))
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
    use super::Seq;
    use super::Version;

    const SINGLE: u8 = 0x80;
    const LIST: u8 = 0xc0;
    use super::REQUEST_LEN;
    use super::REQUEST;
    use super::ALLOWED_LEN;
    use super::ALLOWED;
    use super::DENIED_LEN;
    use super::DENIED;

    const VERSION: Version = 0;

    #[test]
    fn request_rlp_encode() {
        const SEQ: Seq = 0;

        const NONCE: u8 = 32;
        let nonce = NONCE.rlp_bytes();

        let req = Message::connection_request(SEQ, nonce.clone().into_vec());
        let bytes = req.rlp_bytes();
        assert_eq!(1
                   + 1 /* version */ + 1 /* seq */
                   + 1 + REQUEST_LEN as usize /* name */
                   + nonce.len() /* rlp(nonce) */, bytes.len());

        // length prefix
        assert_eq!(LIST as usize + bytes.len() - 1, bytes[0] as usize);

        // version
        assert_eq!(SINGLE as Version + VERSION, bytes[1] as Version);

        // seq
        assert_eq!(SINGLE as Seq + SEQ, bytes[2] as Seq);

        // name
        assert_eq!(SINGLE + REQUEST_LEN, bytes[3]);
        const START_OF_TYPE: usize = 4;
        const START_OF_NONCE: usize = START_OF_TYPE + REQUEST_LEN as usize;
        assert_eq!(REQUEST.as_bytes(), &bytes[START_OF_TYPE..START_OF_NONCE]);

        // nonce
        assert_eq!(nonce.into_vec().as_slice(), &bytes[START_OF_NONCE..]);
    }

    #[test]
    fn allowed_rlp_encode() {
        const SEQ: Seq = 37;

        const NONCE: Nonce = 4;
        let nonce = NONCE.rlp_bytes();

        let allowed = Message::connection_allowed(SEQ, nonce.clone().into_vec());

        let bytes = allowed.rlp_bytes();
        assert_eq!(1 /* version */ + 1 /* seq */
                       + 1 + ALLOWED_LEN as usize /* name */
                       + 1 + nonce.len() /* rlp(nonce) */, bytes.len());

        // length prefix
        assert_eq!(LIST as usize + bytes.len() - 1, bytes[0] as usize);

        // version
        assert_eq!(SINGLE as Version + VERSION, bytes[1] as Version);

        // seq
        assert_eq!(SEQ, bytes[2] as Seq);

        // name
        assert_eq!(SINGLE + ALLOWED_LEN, bytes[3]);
        const START_OF_TYPE: usize = 4;
        const START_OF_NONCE: usize = START_OF_TYPE + ALLOWED_LEN as usize;

        assert_eq!(ALLOWED.as_bytes(), &bytes[START_OF_TYPE..START_OF_NONCE]);

        assert_eq!(nonce.into_vec().as_slice(), &bytes[START_OF_NONCE..]);
    }

    #[test]
    fn denied_rlp_encode() {
        const SEQ: Seq = 6;

        const REASON: &str = "connection denied";
        let reason_len: usize = REASON.len();

        let denied = Message::connection_denied(SEQ, REASON.to_string());

        let bytes = denied.rlp_bytes();
        assert_eq!(1
                       + 1 /* version */ + 1 /* seq */
                       + 1 + DENIED_LEN as usize /* name */
                       + 1 + reason_len /* reason */, bytes.len());

        // length prefix
        assert_eq!(LIST as usize + bytes.len() - 1, bytes[0] as usize);

        // version
        assert_eq!(SINGLE as Version + VERSION, bytes[1] as Version);

        // seq
        assert_eq!(SEQ, bytes[2] as Seq);

        // name
        assert_eq!(SINGLE + DENIED_LEN, bytes[3]);
        const START_OF_TYPE: usize = 4;
        const START_OF_REASON: usize = START_OF_TYPE + DENIED_LEN as usize;

        assert_eq!(DENIED.as_bytes(), &bytes[START_OF_TYPE..START_OF_REASON]);

        assert_eq!(SINGLE + reason_len as u8, bytes[START_OF_REASON]);
        assert_eq!(REASON.as_bytes(), &bytes[(START_OF_REASON + 1)..(START_OF_REASON + 1 + reason_len)]);
    }

    #[test]
    fn request_rlp_decode() {
        const NONCE: Nonce = 42;
        const SEQ: Seq = 17;
        let nonce = NONCE.rlp_bytes().into_vec();

        let mut bytes: Vec<u8> = vec![
            LIST + 1 /* version */ + 1 /* seq */
                + 1 + REQUEST_LEN /* name */
                + nonce.len() as u8 /* rlp(nonce) */];

        bytes.push(SINGLE + VERSION as u8);

        bytes.push(SEQ as u8);

        bytes.push(SINGLE + REQUEST_LEN);
        bytes.extend_from_slice(REQUEST.as_bytes());

        bytes.extend_from_slice(nonce.as_slice());

        assert_eq!(1 + 1 /* version */ + 1 /* seq */
                + 1 + REQUEST_LEN as usize /* name */
                + nonce.len() /* rlp(nonce) */, bytes.len());

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
                + 1 + ALLOWED_LEN /* name */
                + nonce.len() as u8 /* rlp(nonce) */];

        bytes.push(SINGLE + VERSION as u8);

        bytes.push(SEQ as u8);

        bytes.push(SINGLE + ALLOWED_LEN);
        bytes.extend_from_slice(ALLOWED.as_bytes());

        bytes.extend_from_slice(nonce.as_slice());

        assert_eq!(1 + 1 /* version */ + 1 /* seq */
                + 1 + ALLOWED_LEN as usize /* name */
                + nonce.len() /* rlp(nonce) */, bytes.len());

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
                + 1 + DENIED_LEN as u8 /* name */
                + 1 + reason_len as u8 /* reason */];

        bytes.push(SINGLE + VERSION as u8);

        bytes.push(SEQ as u8);

        bytes.push(SINGLE + DENIED_LEN);
        bytes.extend_from_slice(DENIED.as_bytes());

        bytes.extend_from_slice(REASON.rlp_bytes().into_vec().as_slice());

        assert_eq!(1 + 1 /* version */ + 1 /* seq */
                + 1 + DENIED_LEN as usize /* name */
                + 1 + reason_len, bytes.len());

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
        assert_eq!(1
                   + 1 /* version */ + 1 /* seq */
                   + 1 + REQUEST_LEN as usize /* name */
                   + 1 + nonce.len() /* rlp(nonce) */, bytes.len());

        // length prefix
        assert_eq!(LIST as usize + bytes.len() - 1, bytes[0] as usize);

        // version
        assert_eq!(SINGLE as Version + VERSION, bytes[1] as Version);

        // seq
        assert_eq!(SINGLE as Seq + SEQ, bytes[2] as Seq);

        // name
        assert_eq!(SINGLE + REQUEST_LEN, bytes[3]);
        const START_OF_TYPE: usize = 4;
        const START_OF_NONCE: usize = START_OF_TYPE + REQUEST_LEN as usize;
        assert_eq!(REQUEST.as_bytes(), &bytes[START_OF_TYPE..START_OF_NONCE]);

        // nonce
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
        assert_eq!(1
                   +1 /* version */ + 1 /* seq */
                   + 1 + ALLOWED_LEN as usize /* name */
                   + 1 + nonce.len() /* rlp(nonce) */, bytes.len());

        // length prefix
        assert_eq!(LIST as usize + bytes.len() - 1, bytes[0] as usize);

        // version
        assert_eq!(SINGLE as Version + VERSION, bytes[1] as Version);

        // seq
        assert_eq!(SEQ, bytes[2] as Seq);

        // name
        assert_eq!(SINGLE + ALLOWED_LEN, bytes[3]);
        const START_OF_TYPE: usize = 4;
        const START_OF_NONCE: usize = START_OF_TYPE + ALLOWED_LEN as usize;

        assert_eq!(ALLOWED.as_bytes(), &bytes[START_OF_TYPE..START_OF_NONCE]);

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
                + 1 + REQUEST_LEN /* name */
                + 1 + nonce.len() as u8 /* rlp(nonce) */];

        bytes.push(SINGLE + VERSION as u8);

        bytes.push(SEQ as u8);

        bytes.push(SINGLE + REQUEST_LEN);
        bytes.extend_from_slice(REQUEST.as_bytes());

        bytes.push(SINGLE + nonce.len() as u8);
        bytes.extend_from_slice(nonce.as_slice());

        assert_eq!(1 + 1 /* version */ + 1 /* seq */
                + 1 + REQUEST_LEN as usize /* name */
                + 1 + nonce.len() /* rlp(nonce) */, bytes.len());

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
                + 1 + REQUEST_LEN /* name */
                + 1 + nonce.len() as u8 /* rlp(nonce) */];

        bytes.push(SINGLE + VERSION as u8);

        bytes.push(SEQ as u8);

        bytes.push(SINGLE + ALLOWED_LEN);
        bytes.extend_from_slice(ALLOWED.as_bytes());

        bytes.push(SINGLE + nonce.len() as u8);
        bytes.extend_from_slice(nonce.as_slice());

        assert_eq!(1 + 1 /* version */ + 1 /* seq */
                + 1 + ALLOWED_LEN as usize /* name */
                + 1 + nonce.len() /* rlp(nonce) */, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::connection_allowed(SEQ, nonce), message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }
}

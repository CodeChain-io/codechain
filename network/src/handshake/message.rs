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
    Ping(Nonce),
    Pong(Nonce),
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            &Message::Ping(nonce) => {
                s.begin_list(2)
                    .append(&"ping")
                    .append(&nonce)
            },
            &Message::Pong(nonce) => {
                s.begin_list(2)
                    .append(&"pong")
                    .append(&nonce)
            },
        };
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let message: String = rlp.val_at(0)?;
        let nonce: Nonce = rlp.val_at(1)?;
        match message.as_ref() {
            "ping" =>
                Ok(Message::Ping(nonce)),
            "pong" =>
                Ok(Message::Pong(nonce)),
            _ =>
                Err(DecoderError::Custom("Invalid data")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rlp::{ Decodable, Encodable, UntrustedRlp };

    use super::Message;
    use super::Nonce;

    #[test]
    fn ping_rlp_encode() {
        const NONCE: Nonce = 32;
        let ping = Message::Ping(NONCE);
        let bytes = ping.rlp_bytes();
        assert_eq!(7, bytes.len());
        assert_eq!(0xc6, bytes[0]);
        assert_eq!(0x84, bytes[1]);
        const TYPE: &[u8; 4] = b"ping";
        assert_eq!(TYPE, &bytes[2..(2 + TYPE.len())]);
        assert_eq!(NONCE as u8, bytes[6]);
    }

    #[test]
    fn pong_rlp_encode() {
        const NONCE: Nonce = 4;
        let pong = Message::Pong(NONCE);
        let bytes = pong.rlp_bytes();
        assert_eq!(7, bytes.len());
        assert_eq!(0xc6, bytes[0]);
        assert_eq!(0x84, bytes[1]);
        const TYPE: &[u8; 4] = b"pong";
        assert_eq!(TYPE, &bytes[2..(2 + TYPE.len())]);
        assert_eq!(NONCE as u8, bytes[6]);
    }

    #[test]
    fn ping_rlp_decode() {
        const NONCE: Nonce = 42;
        let mut bytes: Vec<u8> = vec![0xc5, 0x84];
        bytes.extend_from_slice("ping".as_bytes());
        bytes.push(NONCE as u8);
        assert_eq!(7, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::Ping(NONCE), message),
            _ => assert!(false),
        }
    }

    #[test]
    fn pong_rlp_decode() {
        const NONCE: Nonce = 42;
        let mut bytes: Vec<u8> = vec![0xc5, 0x84];
        bytes.extend_from_slice("pong".as_bytes());
        bytes.push(NONCE as u8);
        assert_eq!(7, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::Pong(NONCE), message),
            _ => assert!(false),
        }
    }

    const SINGLE: u8 = 0x80;
    const LIST: u8 = 0xc0;
    #[test]
    fn ping_rlp_encode_with_large_nonce() {
        const NONCE: Nonce = 0xDEADBEEF;
        let ping = Message::Ping(NONCE);
        let bytes = ping.rlp_bytes();
        assert_eq!(11, bytes.len());
        assert_eq!(LIST + 10, bytes[0]);
        assert_eq!(SINGLE + 4, bytes[1]);
        const TYPE: &[u8; 4] = b"ping";
        assert_eq!(TYPE, &bytes[2..(2 + TYPE.len())]);
        assert_eq!(SINGLE + 4, bytes[6]);
        assert_eq!(0xDE, bytes[7]);
        assert_eq!(0xAD, bytes[8]);
        assert_eq!(0xBE, bytes[9]);
        assert_eq!(0xEF, bytes[10]);
    }

    #[test]
    fn pong_rlp_encode_with_large_nonce() {
        const NONCE: Nonce = 0xCCAFEC;
        let pong = Message::Pong(NONCE);
        let bytes = pong.rlp_bytes();
        assert_eq!(10, bytes.len());
        assert_eq!(LIST + 9, bytes[0]);
        assert_eq!(SINGLE + 4, bytes[1]);
        const TYPE: &[u8; 4] = b"pong";
        assert_eq!(TYPE, &bytes[2..(2 + TYPE.len())]);
        assert_eq!(SINGLE + 3, bytes[6]);
        assert_eq!(0xCC, bytes[7]);
        assert_eq!(0xAF, bytes[8]);
        assert_eq!(0xEC, bytes[9]);
    }

    #[test]
    fn ping_rlp_decode_with_large_nonce() {
        const NONCE: Nonce = 0xDEADCAFE;
        let bytes: Vec<u8> = vec![
            LIST + 10
            , SINGLE + 4, 'p' as u8, 'i' as u8, 'n' as u8, 'g' as u8
            , SINGLE + 4, 0xDE, 0xAD, 0xCA, 0xFE];
        assert_eq!(11, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::Ping(NONCE), message),
            err => assert!(false),
        }
    }

    #[test]
    fn pong_rlp_decode_with_large_nonce() {
        const NONCE: Nonce = 0xCCCAFE;
        let bytes: Vec<u8> = vec![
            LIST + 9
            , SINGLE + 4, 'p' as u8, 'o' as u8, 'n' as u8, 'g' as u8
            , SINGLE + 3, 0xCC, 0xCA, 0xFE];
        assert_eq!(10, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(Message::Pong(NONCE), message),
            _ => assert!(false),
        }
    }
}

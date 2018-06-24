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

use cnetwork::SocketAddr;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Debug, PartialEq)]
pub enum Message {
    Request(u8),
    Response(Vec<SocketAddr>),
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Message::Request(len) => {
                s.append(len);
            }
            Message::Response(addresses) => {
                s.append_list(addresses);
            }
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.is_int() {
            Ok(Message::Request(rlp.as_val()?))
        } else {
            Ok(Message::Response(rlp.as_list()?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_and_decode_request_0() {
        let request = Message::Request(0);
        let encoded = request.rlp_bytes();
        let rlp = UntrustedRlp::new(&encoded);
        let decoded: Message = Decodable::decode(&rlp).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn encode_and_decode_request_1() {
        let request = Message::Request(1);
        let encoded = request.rlp_bytes();
        let rlp = UntrustedRlp::new(&encoded);
        let decoded: Message = Decodable::decode(&rlp).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn encode_and_decode_request_2() {
        let request = Message::Request(2);
        let encoded = request.rlp_bytes();
        let rlp = UntrustedRlp::new(&encoded);
        let decoded: Message = Decodable::decode(&rlp).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn encode_and_decode_request_3() {
        let request = Message::Request(3);
        let encoded = request.rlp_bytes();
        let rlp = UntrustedRlp::new(&encoded);
        let decoded: Message = Decodable::decode(&rlp).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn encode_and_decode_empty_response() {
        let request = Message::Response(vec![]);
        let encoded = request.rlp_bytes();
        let rlp = UntrustedRlp::new(&encoded);
        let decoded: Message = Decodable::decode(&rlp).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn encode_and_decode_one_response() {
        let request = Message::Response(vec![SocketAddr::v4(127, 0, 0, 1, 3480)]);
        let encoded = request.rlp_bytes();
        let rlp = UntrustedRlp::new(&encoded);
        let decoded: Message = Decodable::decode(&rlp).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn encode_and_decode_two_response() {
        let request = Message::Response(vec![SocketAddr::v4(127, 0, 0, 1, 3480), SocketAddr::v4(127, 0, 0, 1, 3481)]);
        let encoded = request.rlp_bytes();
        let rlp = UntrustedRlp::new(&encoded);
        let decoded: Message = Decodable::decode(&rlp).unwrap();
        assert_eq!(request, decoded);
    }
}

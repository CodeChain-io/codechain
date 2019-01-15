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
                s.append_single_value(len);
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
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn encode_and_decode_request_0() {
        rlp_encode_and_decode_test!(Message::Request(0));
    }

    #[test]
    fn encode_and_decode_request_1() {
        rlp_encode_and_decode_test!(Message::Request(1));
    }

    #[test]
    fn encode_and_decode_request_2() {
        rlp_encode_and_decode_test!(Message::Request(2));
    }

    #[test]
    fn encode_and_decode_request_3() {
        rlp_encode_and_decode_test!(Message::Request(3));
    }

    #[test]
    fn encode_and_decode_empty_response() {
        rlp_encode_and_decode_test!(Message::Response(vec![]));
    }

    #[test]
    fn encode_and_decode_one_response() {
        rlp_encode_and_decode_test!(Message::Response(vec![SocketAddr::v4(127, 0, 0, 1, 3480)]));
    }

    #[test]
    fn encode_and_decode_two_response() {
        rlp_encode_and_decode_test!(Message::Response(vec![
            SocketAddr::v4(127, 0, 0, 1, 3480),
            SocketAddr::v4(127, 0, 0, 1, 3481),
        ]));
    }
}

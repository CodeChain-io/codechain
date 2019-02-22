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

use super::Version;

use super::REQUEST_ID;
use super::RESPONSE_ID;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Message {
    Request {
        extension_name: String,
        extension_versions: Vec<Version>,
    },
    Response {
        extension_name: String,
        allowed_version: Version,
    },
}

impl Message {
    pub fn request(extension_name: String, extension_versions: Vec<Version>) -> Self {
        Message::Request {
            extension_name,
            extension_versions,
        }
    }

    pub fn allowed(extension_name: String, allowed_version: Version) -> Self {
        Message::Response {
            extension_name,
            allowed_version,
        }
    }
}


impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Message::Request {
                extension_name,
                extension_versions,
            } => {
                s.begin_list(3).append(&REQUEST_ID).append(extension_name).append_list(extension_versions);
            }
            Message::Response {
                extension_name,
                allowed_version,
            } => {
                s.begin_list(3).append(&RESPONSE_ID).append(extension_name).append(allowed_version);
            }
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let item_count = rlp.item_count()?;
        if item_count != 3 {
            return Err(DecoderError::RlpInvalidLength {
                expected: 3,
                got: item_count,
            })
        }
        match rlp.val_at(0)? {
            REQUEST_ID => Ok(Message::Request {
                extension_name: rlp.val_at(1)?,
                extension_versions: rlp.list_at(2)?,
            }),
            RESPONSE_ID => Ok(Message::Response {
                extension_name: rlp.val_at(1)?,
                allowed_version: rlp.val_at(2)?,
            }),
            _ => Err(DecoderError::Custom("Invalid id in negotiation message")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn encode_and_decode_request() {
        let extension_name = "some-extension".to_string();
        rlp_encode_and_decode_test!(Message::request(extension_name, vec![1, 2, 3]));
    }

    #[test]
    fn encode_and_decode_allowed() {
        let extension_name = "some-extension".to_string();
        rlp_encode_and_decode_test!(Message::allowed(extension_name, 2));
    }
}

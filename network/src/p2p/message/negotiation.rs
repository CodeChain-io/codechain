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

use super::ProtocolId;
use super::Seq;
use super::Version;

use super::ALLOWED_ID;
use super::DENIED_ID;
use super::REQUEST_ID;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Message {
    version: Version,
    seq: Seq,
    body: Body,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Body {
    Request {
        extension_name: String,
        extension_versions: Vec<Version>,
    },
    Allowed(Version),
    Denied,
}

const COMMON: usize = 3;

impl Message {
    pub fn request(seq: Seq, extension_name: String, extension_versions: Vec<Version>) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::Request {
                extension_name,
                extension_versions,
            },
        }
    }

    pub fn allowed(seq: Seq, version: Version) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::Allowed(version),
        }
    }

    #[allow(dead_code)]
    pub fn denied(seq: Seq) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::Denied,
        }
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn seq(&self) -> Seq {
        self.seq
    }

    pub fn protocol_id(&self) -> ProtocolId {
        match self.body {
            Body::Request {
                ..
            } => REQUEST_ID,
            Body::Allowed(_) => ALLOWED_ID,
            Body::Denied => DENIED_ID,
        }
    }

    fn item_count(&self) -> usize {
        match self.body {
            Body::Request {
                ..
            } => COMMON + 2,
            Body::Allowed(_) => COMMON + 1,
            Body::Denied => COMMON,
        }
    }

    pub fn body(&self) -> &Body {
        &self.body
    }
}


impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(self.item_count()).append(&self.version()).append(&self.protocol_id()).append(&self.seq());

        match &self.body {
            Body::Request {
                extension_name,
                extension_versions,
                ..
            } => {
                s.append(extension_name).append_list(extension_versions);
            }
            Body::Allowed(version) => {
                s.append(version);
            }
            Body::Denied => {}
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let version: Version = rlp.val_at(0)?;
        let protocol_id: ProtocolId = rlp.val_at(1)?;
        let seq: Seq = rlp.val_at(2)?;
        match protocol_id {
            REQUEST_ID => {
                let extension_name: String = rlp.val_at(COMMON)?;
                let extension_versions: Vec<Version> = rlp.list_at(COMMON + 1)?;
                Ok(Message {
                    version,
                    seq,
                    body: Body::Request {
                        extension_name,
                        extension_versions,
                    },
                })
            }
            ALLOWED_ID => Ok(Message {
                version,
                seq,
                body: Body::Allowed(rlp.val_at(COMMON)?),
            }),
            DENIED_ID => {
                let item_count = rlp.item_count()?;
                let mut versions: Vec<Version> = Vec::with_capacity(item_count - COMMON);
                for i in COMMON..item_count {
                    let version: Version = rlp.val_at(i)?;
                    versions.push(version);
                }
                Ok(Message {
                    version,
                    seq,
                    body: Body::Denied,
                })
            }
            _ => Err(DecoderError::Custom("invalid protocol id")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_id_of_request_is_2() {
        assert_eq!(0x02, Message::request(Default::default(), Default::default(), Default::default()).protocol_id());
    }

    #[test]
    fn protocol_id_of_allowed_is_3() {
        assert_eq!(0x03, Message::allowed(Default::default(), Default::default()).protocol_id());
    }

    #[test]
    fn protocol_id_of_denied_is_4() {
        assert_eq!(0x04, Message::denied(Default::default()).protocol_id());
    }

    #[test]
    fn encode_and_decode_request() {
        const SEQ: Seq = 0x5432;
        let extension_name = "some-extension".to_string();
        rlp_encode_and_decode_test!(Message::request(SEQ, extension_name, vec![1, 2, 3]));
    }

    #[test]
    fn encode_and_decode_allowed() {
        const SEQ: Seq = 0x716216a8b1;
        const VERSION: Version = 2;
        rlp_encode_and_decode_test!(Message::allowed(SEQ, VERSION));
    }

    #[test]
    fn encode_and_decode_denied() {
        const SEQ: Seq = 0x3712;
        rlp_encode_and_decode_test!(Message::denied(SEQ));
    }
}

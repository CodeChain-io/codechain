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
        extension_version: Version,
    },
    Allowed,
    Denied(Vec<Version>),
}

const COMMON: usize = 3;

impl Message {
    pub fn request(seq: Seq, extension_name: String, extension_version: Version) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::Request {
                extension_name,
                extension_version,
            },
        }
    }

    pub fn allowed(seq: Seq) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::Allowed,
        }
    }

    #[allow(dead_code)]
    pub fn denied(seq: Seq, versions: Vec<Version>) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::Denied(versions),
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
            Body::Allowed => ALLOWED_ID,
            Body::Denied(_) => DENIED_ID,
        }
    }

    fn item_count(&self) -> usize {
        match self.body {
            Body::Request {
                ..
            } => COMMON + 2,
            Body::Allowed => COMMON,
            Body::Denied(ref versions) => COMMON as usize + versions.len(),
        }
    }

    pub fn body(&self) -> &Body {
        &self.body
    }
}


impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(self.item_count()).append(&self.version()).append(&self.protocol_id()).append(&self.seq());

        match self.body {
            Body::Request {
                ref extension_name,
                extension_version,
                ..
            } => {
                s.append(extension_name).append(&extension_version);
            }
            Body::Allowed => {}
            Body::Denied(ref versions) => {
                for version in versions.iter() {
                    s.append(version);
                }
            }
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
                let extension_version: Version = rlp.val_at(COMMON + 1)?;
                Ok(Message {
                    version,
                    seq,
                    body: Body::Request {
                        extension_name,
                        extension_version,
                    },
                })
            }
            ALLOWED_ID => Ok(Message {
                version,
                seq,
                body: Body::Allowed,
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
                    body: Body::Denied(versions),
                })
            }
            _ => Err(DecoderError::Custom("invalid protocol id")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rlp::{Decodable, Encodable, UntrustedRlp};

    use super::Message;
    use super::Seq;
    use super::Version;

    const SINGLE: u8 = 0x80;
    const LIST: u8 = 0xc0;

    #[test]
    fn protocol_id_of_request_is_2() {
        assert_eq!(0x02, Message::request(Default::default(), Default::default(), Default::default()).protocol_id());
    }

    #[test]
    fn protocol_id_of_allowed_is_3() {
        assert_eq!(0x03, Message::allowed(Default::default()).protocol_id());
    }

    #[test]
    fn protocol_id_of_denied_is_4() {
        assert_eq!(0x04, Message::denied(Default::default(), Default::default()).protocol_id());
    }

    #[test]
    fn encode_request() {
        const SEQ: Seq = 0x5432;
        let extension_name = "some-extension".to_string();
        const EXTENSION_VERSION: Version = 63;

        let request = Message::request(SEQ, extension_name.clone(), EXTENSION_VERSION);
        let result = request.rlp_bytes();

        let length = 1 /* version */ + 1 /* protocol id */ + 1 + 2 /* seq */
            + 1 + extension_name.len() /* extension name */
            + 1 /* extension version */;

        assert_eq!(1 /* prefix */ + length, result.len());

        // length prefix
        assert_eq!(LIST + length as u8, result[0]);

        // version
        assert_eq!(SINGLE + request.version() as u8, result[1]);

        // protocol id
        assert_eq!(request.protocol_id() as u8, result[2]);

        // seq
        assert_eq!(SINGLE + 2 as u8, result[3]);
        assert_eq!(0x54 as u8, result[4]);
        assert_eq!(0x32 as u8, result[5]);

        // extension name
        assert_eq!(SINGLE + extension_name.len() as u8, result[6]);
        assert_eq!(extension_name.as_bytes(), &result[7..(7 + extension_name.len())]);

        // extension version
        assert_eq!(EXTENSION_VERSION as u8, result[7 + extension_name.len()]);
    }

    #[test]
    fn encode_allowed() {
        const SEQ: Seq = 0x3712;

        let allowed = Message::allowed(SEQ);
        let result = allowed.rlp_bytes();

        let length = 1 /* version */ + 1 /* protocol id */ + 1 + 2 /* seq */;

        assert_eq!(1 /* prefix */ + length, result.len());

        // length prefix
        assert_eq!(LIST + length as u8, result[0]);

        // version
        assert_eq!(SINGLE + allowed.version() as u8, result[1]);

        // protocol id
        assert_eq!(allowed.protocol_id() as u8, result[2]);

        // seq
        assert_eq!(SINGLE + 2 as u8, result[3]);
        assert_eq!(0x37 as u8, result[4]);
        assert_eq!(0x12 as u8, result[5]);
    }

    #[test]
    fn encode_denied_without_alternatives() {
        const SEQ: Seq = 0x3712;

        let denied = Message::denied(SEQ, Vec::new());
        let result = denied.rlp_bytes();

        let length = 1 /* version */ + 1 /* protocol id */ + 1 + 2 /* seq */;

        assert_eq!(1 /* prefix */ + length, result.len());

        // length prefix
        assert_eq!(LIST + length as u8, result[0]);

        // version
        assert_eq!(SINGLE + denied.version() as u8, result[1]);

        // protocol id
        assert_eq!(denied.protocol_id() as u8, result[2]);

        // seq
        assert_eq!(SINGLE + 2 as u8, result[3]);
        assert_eq!(0x37 as u8, result[4]);
        assert_eq!(0x12 as u8, result[5]);
    }

    #[test]
    fn encode_denied_with_alternatives() {
        const SEQ: Seq = 0x3712;

        let versions: Vec<Version> = vec![0x10, 0x63, 0x71];

        let denied = Message::denied(SEQ, versions.clone());
        let result = denied.rlp_bytes();

        let length = 1 /* version */ + 1 /* protocol id */ + 1 + 2 /* seq */
            + versions.len() /* versions */;

        assert_eq!(1 /* prefix */ + length, result.len());

        // length prefix
        assert_eq!(LIST + length as u8, result[0]);

        // version
        assert_eq!(SINGLE + denied.version() as u8, result[1]);

        // protocol id
        assert_eq!(denied.protocol_id() as u8, result[2]);

        // seq
        assert_eq!(SINGLE + 2 as u8, result[3]);
        assert_eq!(0x37 as u8, result[4]);
        assert_eq!(0x12 as u8, result[5]);

        // versions
        assert_eq!(0x10 as u8, result[6]);
        assert_eq!(0x63 as u8, result[7]);
        assert_eq!(0x71 as u8, result[8]);
    }

    #[test]
    fn decode_request() {
        const SEQ: Seq = 0x16a8b1;
        let extension_name = "some-extension".to_string();
        const EXTENSION_VERSION: Version = 63;

        let length = 1 /* version */ + 1 /* protocol id */ + 1 + 3 /* seq */
            + 1 + extension_name.len() /* extension name */
            + 1 /* extension version */;

        let mut bytes = vec![LIST + length as u8];
        // version
        bytes.push(SINGLE + 0);

        // protocol id
        bytes.push(0x02);

        // seq
        bytes.push(SINGLE + 3);
        bytes.push(0x16);
        bytes.push(0xa8);
        bytes.push(0xb1);

        // extension name
        bytes.push(SINGLE + extension_name.len() as u8);
        bytes.extend_from_slice(extension_name.as_bytes());

        // extension version
        bytes.push(EXTENSION_VERSION as u8);

        assert_eq!(length + 1, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(actual) => {
                let expected = Message::request(SEQ, extension_name.clone(), EXTENSION_VERSION);
                assert_eq!(expected, actual)
            }
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn decode_allowed() {
        const SEQ: Seq = 0x716216a8b1;

        let length = 1 /* version */ + 1 /* protocol id */ + 1 + 5 /* seq */;

        let mut bytes = vec![LIST + length as u8];
        // version
        bytes.push(SINGLE + 0);

        // protocol id
        bytes.push(0x03);

        // seq
        bytes.push(SINGLE + 5);
        bytes.push(0x71);
        bytes.push(0x62);
        bytes.push(0x16);
        bytes.push(0xa8);
        bytes.push(0xb1);

        assert_eq!(length + 1, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(actual) => {
                let expected = Message::allowed(SEQ);
                assert_eq!(expected, actual)
            }
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn decode_denied_without_alternatives() {
        const SEQ: Seq = 0x716216a8b1c910;

        let length = 1 /* version */ + 1 /* protocol id */ + 1 + 7 /* seq */;

        let mut bytes = vec![LIST + length as u8];
        // version
        bytes.push(SINGLE + 0);

        // protocol id
        bytes.push(0x04);

        // seq
        bytes.push(SINGLE + 7);
        bytes.push(0x71);
        bytes.push(0x62);
        bytes.push(0x16);
        bytes.push(0xa8);
        bytes.push(0xb1);
        bytes.push(0xc9);
        bytes.push(0x10);

        assert_eq!(length + 1, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(actual) => {
                let expected = Message::denied(SEQ, Vec::new());
                assert_eq!(expected, actual)
            }
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn decode_denied_with_alternatives() {
        const SEQ: Seq = 0x716216a8b1c910;

        let versions: Vec<Version> = vec![0x01, 0x02, 0x03, 0x68231f];

        let length = 1 /* version */ + 1 /* protocol id */ + 1 + 7 /* seq */
            + versions.len() - 1 + 1 + 3 /* versions */;

        let mut bytes = vec![LIST + length as u8];
        // version
        bytes.push(SINGLE + 0);

        // protocol id
        bytes.push(0x04);

        // seq
        bytes.push(SINGLE + 7);
        bytes.push(0x71);
        bytes.push(0x62);
        bytes.push(0x16);
        bytes.push(0xa8);
        bytes.push(0xb1);
        bytes.push(0xc9);
        bytes.push(0x10);

        for version in versions.iter() {
            bytes.extend_from_slice(version.rlp_bytes().into_vec().as_slice());
        }

        assert_eq!(length + 1, bytes.len());

        let rlp = UntrustedRlp::new(&bytes);

        match Decodable::decode(&rlp) {
            Ok(actual) => {
                let expected = Message::denied(SEQ, versions);
                assert_eq!(expected, actual)
            }
            Err(err) => assert!(false, "{:?}", err),
        }
    }
}

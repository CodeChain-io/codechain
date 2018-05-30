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

use super::super::NodeId;

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
    NodeIdRequest(NodeId),
    NodeIdResponse(NodeId),
    ConnectionRequest(Raw),
    ConnectionAllowed(Raw),
    ConnectionDenied(String),
    EcdhRequest(Public),
    EcdhAllowed(Public),
    EcdhDenied(String),
}

const NODE_ID_REQUEST: u8 = 0x01;
const NODE_ID_RESPONSE: u8 = 0x02;

const CONNECTION_REQUEST: u8 = 0x3;
const CONNECTION_ALLOWED: u8 = 0x4;
const CONNECTION_DENIED: u8 = 0x5;

const ECDH_REQUEST: u8 = 0x06;
const ECDH_ALLOWED: u8 = 0x07;
const ECDH_DENIED: u8 = 0x08;

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

    pub fn node_id_request(seq: Seq, id: NodeId) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::NodeIdRequest(id),
        }
    }

    pub fn node_id_response(seq: Seq, id: NodeId) -> Self {
        Self {
            version: 0,
            seq,
            body: Body::NodeIdResponse(id),
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
            Body::NodeIdRequest(_) => NODE_ID_REQUEST,
            Body::NodeIdResponse(_) => NODE_ID_RESPONSE,
        }
    }

    pub fn body(&self) -> &Body {
        &self.body
    }

    pub fn seq(&self) -> Seq {
        self.seq
    }

    #[allow(dead_code)]
    pub fn version(&self) -> Version {
        self.version
    }

    fn item_count(&self) -> usize {
        4
    }
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        let version = self.version;
        let seq = self.seq;
        s.begin_list(self.item_count()).append(&version).append(&seq).append(&self.protocol_id());
        match &self.body {
            Body::NodeIdRequest(id) => {
                s.append(id);
            }
            Body::NodeIdResponse(id) => {
                s.append(id);
            }
            Body::ConnectionRequest(body) => {
                s.append(body);
            }
            Body::ConnectionAllowed(body) => {
                s.append(body);
            }
            Body::ConnectionDenied(reason) => {
                s.append(reason);
            }
            Body::EcdhRequest(key) => {
                s.append(key);
            }
            Body::EcdhAllowed(key) => {
                s.append(key);
            }
            Body::EcdhDenied(reason) => {
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
        let message = match protocol_id {
            NODE_ID_REQUEST => Message::node_id_request(seq, rlp.val_at(3)?),
            NODE_ID_RESPONSE => {
                let node_id = rlp.val_at(3)?;
                Message::node_id_response(seq, node_id)
            }
            CONNECTION_REQUEST => {
                let body: Raw = rlp.val_at(3)?;
                Message::connection_request(seq, body)
            }
            CONNECTION_ALLOWED => {
                let body: Raw = rlp.val_at(3)?;
                Message::connection_allowed(seq, body)
            }
            CONNECTION_DENIED => {
                let reason: String = rlp.val_at(3)?;
                Message::connection_denied(seq, reason)
            }
            ECDH_REQUEST => {
                let key: Public = rlp.val_at(3)?;
                Message::ecdh_request(seq, key)
            }
            ECDH_ALLOWED => {
                let key: Public = rlp.val_at(3)?;
                Message::ecdh_allowed(seq, key)
            }
            ECDH_DENIED => {
                let reason: String = rlp.val_at(3)?;
                Message::ecdh_denied(seq, reason)
            }
            _ => return Err(DecoderError::Custom("Invalid protocol id")),
        };
        if message.item_count() != rlp.item_count()? {
            return Err(DecoderError::RlpInvalidLength)
        }
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use rlp::{Decodable, Encodable, UntrustedRlp};

    use super::super::super::session::Nonce;
    use super::*;

    #[test]
    fn encode_and_decode_request() {
        const SEQ: Seq = 0;

        let nonce = Nonce::from(32);
        let nonce = nonce.rlp_bytes();

        let req = Message::connection_request(SEQ, nonce.clone().into_vec());
        let bytes = req.rlp_bytes();

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(req, message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn encode_and_decode_allowed() {
        const SEQ: Seq = 37;

        let nonce = Nonce::from(4);
        let nonce = nonce.rlp_bytes();

        let allowed = Message::connection_allowed(SEQ, nonce.clone().into_vec());

        let bytes = allowed.rlp_bytes();

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(allowed, message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn encode_and_decode_denied() {
        const SEQ: Seq = 6;

        const REASON: &str = "connection denied";

        let denied = Message::connection_denied(SEQ, REASON.to_string());

        let bytes = denied.rlp_bytes();

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(denied, message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn encode_and_decode_request_with_large_nonce() {
        let nonce = Nonce::from(0xDEADBEEF);
        let nonce = nonce.rlp_bytes();

        const SEQ: Seq = 0;

        let req = Message::connection_request(SEQ, nonce.clone().into_vec());
        let bytes = req.rlp_bytes();

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(req, message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn encode_and_decode_allowed_with_large_nonce() {
        let nonce = Nonce::from(0xCCAFEC);
        let nonce = nonce.rlp_bytes();

        const SEQ: Seq = 0x4a;

        let allowed = Message::connection_allowed(SEQ, nonce.clone().into_vec());
        let bytes = allowed.rlp_bytes();

        let rlp = UntrustedRlp::new(&bytes);
        match Decodable::decode(&rlp) {
            Ok(message) => assert_eq!(allowed, message),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn encode_and_decode_node_id_request() {
        let node_id = 0xBEEFCAFE.into();
        let request = Message::node_id_request(0x8a, node_id);

        let encoded = request.rlp_bytes();
        let rlp = UntrustedRlp::new(&encoded);
        match Decodable::decode(&rlp) {
            Ok(decoded) => assert_eq!(request, decoded),
            Err(err) => assert!(false, "{:?}", err),
        }
    }

    #[test]
    fn encode_and_decode_node_id_response() {
        let id = NodeId::random();
        let response = Message::node_id_response(0x9a, id);

        let encoded = response.rlp_bytes();
        let rlp = UntrustedRlp::new(&encoded);
        match Decodable::decode(&rlp) {
            Ok(decoded) => assert_eq!(response, decoded),
            Err(err) => assert!(false, "{:?}", err),
        }
    }
}

// Copyright 2019 Kodebox, Inc.
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

use ibc;
use ibc::connection_03::connection_path;
use ibc::KVStore;
use rlp;
use rlp::{DecoderError, UntrustedRlp};

pub enum ConnectionState {
    INIT,
    TRYOPEN,
    OPEN,
}

pub enum ConnectionVersion {
    Pick(String),
    Compatible(Vec<String>),
}

#[derive(RlpEncodable, RlpDecodable)]
pub struct ConnectionEnd {
    state: ConnectionState,
    counterparty_connection_id: String,
    // NOTE: counterparty_prefix is required according to the spec.
    client_id: String,
    counterparty_client_id: String,
    version: ConnectionVersion,
}

impl ConnectionEnd {
    pub fn new_init(
        ctx: &mut dyn ibc::Context,
        id: String,
        client_id: String,
        counterparty_connection_id: String,
        counterparty_client_id: String,
        version: ConnectionVersion,
    ) -> Self {
        let c = ConnectionEnd {
            state: ConnectionState::INIT,
            client_id,
            counterparty_connection_id,
            counterparty_client_id,
            version: ConnectionVersion::Compatible(vec!["0".to_owned()]),
        };
        c.set_connection(ctx, id);
        c
    }

    fn set_connection(&self, ctx: &mut dyn ibc::Context, id: String) {
        let kv_store = ctx.get_kv_store();
        let path = connection_path(&id);
        KVStore::set(kv_store, &path, &rlp::encode(self).into_vec());
    }
}

/*
impl rlp::Encodable for ConnectionEnd {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(5);
        s.append(&self.state);
        s.append(&self.counterparty_connection_id);
        s.append(&self.client_id);
        s.append(&self.counterparty_client_id);
        s.append(&self.version);
    }
}

impl rlp::Decodable for ConnectionEnd {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let item_count = rlp.item_count()?;
        let expected = 3;
        if item_count != expected {
            return Err(DecoderError::RlpIncorrectListLen {
                expected,
                got: item_count,
            })
        }
        Ok(ConnectionEnd {
            state: rlp.val_at(0)?,
            counterparty_connection_id: rlp.val_at(1)?,
            client_id: rlp.val_at(2)?,
            counterparty_client_id: rlp.val_at(3)?,
            version: rlp.val_at(4)?,
        })
    }
}
*/

impl rlp::Encodable for ConnectionState {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        match *self {
            ConnectionState::INIT => s.append::<u8>(&0),
            ConnectionState::TRYOPEN => s.append::<u8>(&1),
            ConnectionState::OPEN => s.append::<u8>(&2),
        };
    }
}

impl rlp::Decodable for ConnectionState {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if !rlp.is_data() {
            return Err(DecoderError::RlpExpectedToBeData)
        }
        match rlp.val_at::<u8>(0)? {
            0 => Ok(ConnectionState::INIT),
            1 => Ok(ConnectionState::TRYOPEN),
            2 => Ok(ConnectionState::OPEN),
            value => Err(DecoderError::Custom(&format!("Invalid value for ConnectionState: {}", value))),
        }
    }
}

impl rlp::Encodable for ConnectionVersion {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        s.begin_list(1);
        match self {
            ConnectionVersion::Compatible(versions) => s.append_list(versions),
            ConnectionVersion::Pick(version) => s.append(version),
        };
    }
}

impl rlp::Decodable for ConnectionVersion {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        unimplemented!()
    }
}

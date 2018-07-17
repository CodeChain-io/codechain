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

use ccrypto::aes::{self, SymmetricCipherError};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::super::super::session::Session;
use super::ProtocolId;
use super::Version;

use super::ENCRYPTED_ID;
use super::UNENCRYPTED_ID;


#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Message {
    version: Version,
    extension_name: String,
    extension_version: Version,
    data: Data,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Data {
    Encrypted(Vec<u8>),
    Unencrypted(Vec<u8>),
}

impl Message {
    #[allow(dead_code)]
    pub fn encrypted(extension_name: String, extension_version: Version, data: &[u8]) -> Self {
        Self {
            version: 0,
            extension_name,
            extension_version,
            data: Data::Encrypted(data.to_vec()),
        }
    }

    pub fn encrypted_from_unencrypted_data(
        extension_name: String,
        extension_version: Version,
        unencrypted_data: &[u8],
        session: &Session,
    ) -> Result<Self, SymmetricCipherError> {
        let data = Data::Encrypted(aes::encrypt(unencrypted_data, session.secret(), &session.id().clone().into())?);
        Ok(Self {
            version: 0,
            extension_name,
            extension_version,
            data,
        })
    }
    pub fn unencrypted(extension_name: String, extension_version: Version, data: &[u8]) -> Self {
        Self {
            version: 0,
            extension_name,
            extension_version,
            data: Data::Unencrypted(data.to_vec()),
        }
    }

    pub fn data(&self) -> &[u8] {
        match self.data {
            Data::Encrypted(ref data) => &data,
            Data::Unencrypted(ref data) => &data,
        }
    }

    pub fn unencrypted_data(&self, session: &Session) -> Result<Vec<u8>, SymmetricCipherError> {
        match self.data {
            Data::Encrypted(ref data) => aes::decrypt(&data, session.secret(), &session.id().clone().into()),
            Data::Unencrypted(ref data) => Ok(data.clone()),
        }
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn protocol_id(&self) -> ProtocolId {
        match self.data {
            Data::Encrypted(_) => ENCRYPTED_ID,
            Data::Unencrypted(_) => UNENCRYPTED_ID,
        }
    }

    pub fn extension_name(&self) -> &String {
        &self.extension_name
    }

    pub fn extension_version(&self) -> Version {
        self.extension_version
    }
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(5)
            .append(&self.version())
            .append(&self.protocol_id())
            .append(self.extension_name())
            .append(&self.extension_version())
            .append(&self.data());
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let version: Version = rlp.val_at(0)?;
        let protocol_id: ProtocolId = rlp.val_at(1)?;
        let extension_name: String = rlp.val_at(2)?;
        let extension_version: Version = rlp.val_at(3)?;
        let data: Vec<u8> = rlp.val_at(4)?;
        let data = match protocol_id {
            ENCRYPTED_ID => Data::Encrypted(data),
            UNENCRYPTED_ID => Data::Unencrypted(data),
            _ => return Err(DecoderError::Custom("invalid protocol id")),
        };
        Ok(Self {
            version,
            extension_name,
            extension_version,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use ckey::Secret;
    use rand::{OsRng, Rng};

    use super::super::super::message::Nonce;
    use super::*;

    #[test]
    fn encrypted_id_is_5() {
        assert_eq!(5, super::ENCRYPTED_ID)
    }

    #[test]
    fn unencrypted_id_is_6() {
        assert_eq!(6, super::UNENCRYPTED_ID)
    }

    #[test]
    fn encrypted_with_unencrypted_data_function_internally_encrypts() {
        let extension_name = "encrypt".to_string();
        let extension_version = 3;
        let unencrypted_data = "this data must be encrypted".as_bytes();
        let shared_secret = Secret::random();

        let mut rng = OsRng::new().expect("Cannot generate random number");
        let nonce: Nonce = rng.gen();

        let session = Session::new(shared_secret, nonce);
        let encrypted =
            Message::encrypted_from_unencrypted_data(extension_name, extension_version, &unencrypted_data, &session)
                .unwrap();
        assert_ne!(unencrypted_data, encrypted.data());
        assert_eq!(unencrypted_data, encrypted.unencrypted_data(&session).unwrap().as_slice());
    }
}

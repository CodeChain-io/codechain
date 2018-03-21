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

use ccrypto::aes;
use ctypes::hash::{H128, H256};
use rcrypto::symmetriccipher::SymmetricCipherError;
use rlp::{UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};

type Version = u64;
type ProtocolId = u32;
type SharedSecret = H256;
type Nonce = H128;
type SessionKey = (SharedSecret, Nonce);

const ENCRYPTED_ID: ProtocolId = 0x05;
const UNENCRYPTED_ID: ProtocolId = 0x06;

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Message {
    version: Version,
    application_name: String,
    application_version: Version,
    data: Data,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Data {
    Encrypted(Vec<u8>),
    Unencrypted(Vec<u8>),
}

impl Message {
    pub fn encrypted(application_name: String, application_version: Version, data: Vec<u8>) -> Self {
         Self {
            version: 0,
            application_name,
            application_version,
            data: Data::Encrypted(data),
        }
    }

    pub fn encrypted_from_unencrypted_data(application_name: String, application_version: Version, unencrypted_data: Vec<u8>, session_key: &SessionKey) -> Result<Self, SymmetricCipherError> {
        let data = Data::Encrypted(aes::encrypt(unencrypted_data.as_slice(), &session_key.0, &session_key.1)?);
        Ok(Self {
            version: 0,
            application_name,
            application_version,
            data,
        })
    }
    pub fn unencrypted(application_name: String, application_version: Version, data: Vec<u8>) -> Self {
         Self {
            version: 0,
            application_name,
            application_version,
            data: Data::Unencrypted(data),
        }
    }

    pub fn data(&self) -> &Vec<u8> {
        match &self.data {
            &Data::Encrypted(ref data) => &data,
            &Data::Unencrypted(ref data) => &data,
        }
    }

    pub fn unencrypted_data(&self, session_key: &SessionKey) -> Result<Vec<u8>, SymmetricCipherError> {
        match &self.data {
            &Data::Encrypted(ref data) => aes::decrypt(data.as_slice(), &session_key.0, &session_key.1),
            &Data::Unencrypted(ref data) => Ok(data.clone()),
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

    pub fn application_name(&self) -> &String {
        &self.application_name
    }

    pub fn application_version(&self) -> Version {
        self.application_version
    }
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(5)
            .append(&self.version())
            .append(&self.protocol_id())
            .append(self.application_name())
            .append(&self.application_version())
            .append(self.data());
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let version: Version = rlp.val_at(0)?;
        let protocol_id: ProtocolId = rlp.val_at(1)?;
        let application_name: String = rlp.val_at(2)?;
        let application_version: Version = rlp.val_at(3)?;
        let data: Vec<u8> = rlp.val_at(4)?;
        let data = match protocol_id {
            ENCRYPTED_ID => Data::Encrypted(data),
            UNENCRYPTED_ID => Data::Unencrypted(data),
            _ => return Err(DecoderError::Custom("invalid protocol id")),
        };
        Ok(Self {
            version,
            application_name,
            application_version,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Message;
    use super::Nonce;
    use super::SharedSecret;

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
        let application_name = "encrypt".to_string();
        let application_version = 3;
        let unencrypted_data: Vec<u8> = "this data must be encrypted".as_bytes().to_vec();
        let shared_secret = SharedSecret::random();
        let nonce = Nonce::random();

        let encrypted = Message::encrypted_from_unencrypted_data(application_name, application_version, unencrypted_data.clone(), &(shared_secret, nonce)).unwrap();
        assert_ne!(&unencrypted_data, encrypted.data());
        assert_eq!(unencrypted_data, encrypted.unencrypted_data(&(shared_secret, nonce)).unwrap());
    }
}

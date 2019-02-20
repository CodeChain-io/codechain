// Copyright 2018-2019 Kodebox, Inc.
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

use crate::session::Session;

use super::ENCRYPTED_ID;
use super::UNENCRYPTED_ID;


#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Message {
    Encrypted {
        extension_name: String,
        encrypted: Vec<u8>,
    },
    Unencrypted {
        extension_name: String,
        data: Vec<u8>,
    },
}

impl Message {
    pub fn encrypted(extension_name: String, encrypted: Vec<u8>) -> Self {
        Message::Encrypted {
            extension_name,
            encrypted,
        }
    }

    pub fn encrypted_from_unencrypted_data(
        extension_name: String,
        unencrypted_data: &[u8],
        session: &Session,
    ) -> Result<Self, SymmetricCipherError> {
        let encrypted = aes::encrypt(unencrypted_data, session.secret(), &session.nonce())?;
        Ok(Self::encrypted(extension_name, encrypted))
    }

    pub fn unencrypted(extension_name: String, data: Vec<u8>) -> Self {
        Message::Unencrypted {
            extension_name,
            data,
        }
    }

    #[cfg(test)]
    fn data(&self) -> &[u8] {
        match self {
            Message::Encrypted {
                encrypted,
                ..
            } => &encrypted,
            Message::Unencrypted {
                data,
                ..
            } => &data,
        }
    }

    pub fn unencrypted_data(&self, session: &Session) -> Result<Vec<u8>, SymmetricCipherError> {
        match self {
            Message::Encrypted {
                encrypted,
                ..
            } => aes::decrypt(encrypted, session.secret(), &session.nonce()),
            Message::Unencrypted {
                data,
                ..
            } => Ok(data.clone()),
        }
    }

    pub fn extension_name(&self) -> &str {
        match self {
            Message::Encrypted {
                extension_name,
                ..
            } => &extension_name,
            Message::Unencrypted {
                extension_name,
                ..
            } => &extension_name,
        }
    }
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Message::Encrypted {
                extension_name,
                encrypted,
            } => {
                s.begin_list(3).append(&ENCRYPTED_ID).append(extension_name).append(encrypted);
            }
            Message::Unencrypted {
                extension_name,
                data,
            } => {
                s.begin_list(3).append(&UNENCRYPTED_ID).append(extension_name).append(data);
            }
        }
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let item_count = rlp.item_count()?;
        if item_count != 3 {
            return Err(DecoderError::RlpInvalidLength {
                expected: 5,
                got: item_count,
            })
        }
        match rlp.val_at(0)? {
            ENCRYPTED_ID => Ok(Message::Encrypted {
                extension_name: rlp.val_at(1)?,
                encrypted: rlp.val_at(2)?,
            }),
            UNENCRYPTED_ID => Ok(Message::Unencrypted {
                extension_name: rlp.val_at(1)?,
                data: rlp.val_at(2)?,
            }),
            _ => Err(DecoderError::Custom("Invalid id in extension message")),
        }
    }
}

#[cfg(test)]
mod tests {
    use ckey::Secret;
    use rand::rngs::OsRng;
    use rand::Rng;
    use rlp::rlp_encode_and_decode_test;

    use super::super::super::message::Nonce;
    use super::*;

    #[test]
    fn encrypted_with_unencrypted_data_function_internally_encrypts() {
        let extension_name = "encrypt".to_string();
        let unencrypted_data = b"this data must be encrypted";
        let shared_secret = Secret::random();

        let mut rng = OsRng::new().expect("Cannot generate random number");
        let nonce: Nonce = rng.gen();

        let session = Session::new(shared_secret, nonce);
        let encrypted = Message::encrypted_from_unencrypted_data(extension_name, unencrypted_data, &session).unwrap();
        assert_ne!(unencrypted_data, encrypted.data());
        assert_eq!(unencrypted_data, encrypted.unencrypted_data(&session).unwrap().as_slice());
    }

    #[test]
    fn encode_and_decode_encrypted() {
        rlp_encode_and_decode_test!(Message::encrypted("a".to_string(), vec![1, 2, 3, 4]));
    }

    #[test]
    fn encode_and_decode_unencrypted() {
        rlp_encode_and_decode_test!(Message::unencrypted("a".to_string(), vec![1, 2, 3, 4]));
    }
}

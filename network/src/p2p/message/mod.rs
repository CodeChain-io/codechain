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

mod extension;
mod handshake;
mod message;
mod negotiation;
mod signed_message;

use ctypes::H256;
use ctypes::Secret;

pub use self::extension::Message as ExtensionMessage;
pub use self::handshake::Message as HandshakeMessage;
pub use self::message::Message;
pub use self::negotiation::{Body as NegotiationBody, Message as NegotiationMessage};
pub use self::signed_message::SignedMessage;
pub use super::super::session::Nonce;

pub type Version = u64;
pub type ProtocolId = u64;
pub type Seq = u64;
pub type SessionKey = (Secret, Nonce);
pub type Signature = H256;

pub const SYNC_ID: ProtocolId = 0x00;
pub const ACK_ID: ProtocolId = 0x01;
pub const REQUEST_ID: ProtocolId = 0x02;
pub const ALLOWED_ID: ProtocolId = 0x03;
pub const DENIED_ID: ProtocolId = 0x04;
pub const ENCRYPTED_ID: ProtocolId = 0x05;
pub const UNENCRYPTED_ID: ProtocolId = 0x06;

#[cfg(test)]
mod tests {
    use super::ACK_ID;
    use super::ALLOWED_ID;
    use super::DENIED_ID;
    use super::ENCRYPTED_ID;
    use super::REQUEST_ID;
    use super::SYNC_ID;
    use super::UNENCRYPTED_ID;

    #[test]
    fn sync_id_is_a_unique() {
        assert_ne!(SYNC_ID, ACK_ID);
        assert_ne!(SYNC_ID, REQUEST_ID);
        assert_ne!(SYNC_ID, ALLOWED_ID);
        assert_ne!(SYNC_ID, DENIED_ID);
        assert_ne!(SYNC_ID, ENCRYPTED_ID);
        assert_ne!(SYNC_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn ack_id_is_a_unique() {
        assert_ne!(ACK_ID, SYNC_ID);
        assert_ne!(ACK_ID, REQUEST_ID);
        assert_ne!(ACK_ID, ALLOWED_ID);
        assert_ne!(ACK_ID, DENIED_ID);
        assert_ne!(ACK_ID, ENCRYPTED_ID);
        assert_ne!(ACK_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn request_id_is_a_unique() {
        assert_ne!(REQUEST_ID, SYNC_ID);
        assert_ne!(REQUEST_ID, ACK_ID);
        assert_ne!(REQUEST_ID, ALLOWED_ID);
        assert_ne!(REQUEST_ID, DENIED_ID);
        assert_ne!(REQUEST_ID, ENCRYPTED_ID);
        assert_ne!(REQUEST_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn allowed_id_is_a_unique() {
        assert_ne!(ALLOWED_ID, SYNC_ID);
        assert_ne!(ALLOWED_ID, ACK_ID);
        assert_ne!(ALLOWED_ID, REQUEST_ID);
        assert_ne!(ALLOWED_ID, DENIED_ID);
        assert_ne!(ALLOWED_ID, ENCRYPTED_ID);
        assert_ne!(ALLOWED_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn denied_id_is_a_unique() {
        assert_ne!(DENIED_ID, SYNC_ID);
        assert_ne!(DENIED_ID, ACK_ID);
        assert_ne!(DENIED_ID, REQUEST_ID);
        assert_ne!(DENIED_ID, ALLOWED_ID);
        assert_ne!(DENIED_ID, ENCRYPTED_ID);
        assert_ne!(DENIED_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn encrypted_id_is_a_unique() {
        assert_ne!(ENCRYPTED_ID, SYNC_ID);
        assert_ne!(ENCRYPTED_ID, ACK_ID);
        assert_ne!(ENCRYPTED_ID, REQUEST_ID);
        assert_ne!(ENCRYPTED_ID, ALLOWED_ID);
        assert_ne!(ENCRYPTED_ID, DENIED_ID);
        assert_ne!(ENCRYPTED_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn unencrypted_id_is_a_unique() {
        assert_ne!(UNENCRYPTED_ID, SYNC_ID);
        assert_ne!(UNENCRYPTED_ID, ACK_ID);
        assert_ne!(UNENCRYPTED_ID, REQUEST_ID);
        assert_ne!(UNENCRYPTED_ID, ALLOWED_ID);
        assert_ne!(UNENCRYPTED_ID, DENIED_ID);
        assert_ne!(UNENCRYPTED_ID, ENCRYPTED_ID);
    }
}

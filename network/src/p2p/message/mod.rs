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
#[cfg_attr(feature = "cargo-clippy", allow(clippy::module_inception))]
mod message;
mod negotiation;
mod signed_message;

use primitives::H256;

pub use self::extension::Message as ExtensionMessage;
pub use self::message::Message;
pub use self::negotiation::Message as NegotiationMessage;
pub use self::signed_message::SignedMessage;
pub use crate::session::Nonce;

pub type Version = u64;
pub type Signature = H256;

pub const REQUEST_ID: u8 = 0x05;
pub const RESPONSE_ID: u8 = 0x06;
pub const ENCRYPTED_ID: u8 = 0x07;
pub const UNENCRYPTED_ID: u8 = 0x08;

#[cfg(test)]
mod tests {
    use super::ENCRYPTED_ID;
    use super::REQUEST_ID;
    use super::RESPONSE_ID;
    use super::UNENCRYPTED_ID;

    #[test]
    fn request_id_is_a_unique() {
        assert_ne!(REQUEST_ID, RESPONSE_ID);
        assert_ne!(REQUEST_ID, ENCRYPTED_ID);
        assert_ne!(REQUEST_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn response_id_is_a_unique() {
        assert_ne!(RESPONSE_ID, REQUEST_ID);
        assert_ne!(RESPONSE_ID, ENCRYPTED_ID);
        assert_ne!(RESPONSE_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn encrypted_id_is_a_unique() {
        assert_ne!(ENCRYPTED_ID, REQUEST_ID);
        assert_ne!(ENCRYPTED_ID, RESPONSE_ID);
        assert_ne!(ENCRYPTED_ID, UNENCRYPTED_ID);
    }

    #[test]
    fn unencrypted_id_is_a_unique() {
        assert_ne!(UNENCRYPTED_ID, REQUEST_ID);
        assert_ne!(UNENCRYPTED_ID, RESPONSE_ID);
        assert_ne!(UNENCRYPTED_ID, ENCRYPTED_ID);
    }
}

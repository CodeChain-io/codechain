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

use ckey::{public_to_address, Address, Public};
use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use crate::CacheableItem;

#[derive(Clone, Debug)]
pub struct RegularAccount {
    owner_public: Public,
}

impl RegularAccount {
    pub fn new(owner_public: Public) -> Self {
        Self {
            owner_public,
        }
    }

    pub fn owner_public(&self) -> &Public {
        &self.owner_public
    }

    pub fn set_owner_public(&mut self, owner_public: &Public) {
        self.owner_public = *owner_public;
    }
}

impl Default for RegularAccount {
    fn default() -> Self {
        Self::new(Public::default())
    }
}

impl CacheableItem for RegularAccount {
    type Address = RegularAccountAddress;

    fn is_null(&self) -> bool {
        self.owner_public.is_zero()
    }
}

const PREFIX: u8 = super::REGULAR_ACCOUNT_PREFIX;

impl Encodable for RegularAccount {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2).append(&PREFIX).append(&self.owner_public);
    }
}

impl Decodable for RegularAccount {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? != 2 {
            return Err(DecoderError::RlpInvalidLength)
        }
        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for regular account", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            owner_public: rlp.val_at(1)?,
        })
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegularAccountAddress(H256);

impl_address!(TOP, RegularAccountAddress, PREFIX);

impl RegularAccountAddress {
    pub fn new(public: &Public) -> Self {
        let address = public_to_address(public);
        Self::from_transaction_hash(::ccrypto::blake256(&address), 0)
    }

    pub fn from_address(address: &Address) -> Self {
        Self::from_transaction_hash(::ccrypto::blake256(address), 0)
    }
}

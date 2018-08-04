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

use super::cache::CacheableItem;

#[derive(Clone, Debug)]
pub struct RegularAccount {
    master_account: Public,
}

impl RegularAccount {
    pub fn new(master_account: Public) -> Self {
        Self {
            master_account,
        }
    }

    pub fn master_account(&self) -> &Public {
        &self.master_account
    }

    pub fn set_master_account(&mut self, master_public: &Public) {
        self.master_account = *master_public;
    }
}

impl CacheableItem for RegularAccount {
    type Address = RegularAccountAddress;

    fn is_null(&self) -> bool {
        self.master_account.is_zero()
    }
}

const PREFIX: u8 = super::REGULAR_ACCOUNT_PREFIX;

impl Encodable for RegularAccount {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2).append(&PREFIX).append(&self.master_account);
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
            master_account: rlp.val_at(1)?,
        })
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

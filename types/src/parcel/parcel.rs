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

use ccrypto::blake256;
use ckey::NetworkId;
use heapsize::HeapSizeOf;
use primitives::{H256, U256};
use rlp::RlpStream;

use super::Action;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parcel {
    /// Nonce.
    pub nonce: U256,
    /// Amount of CCC to be paid as a cost for distributing this parcel to the network.
    pub fee: U256,
    /// Network Id
    pub network_id: NetworkId,

    pub action: Action,
}

impl HeapSizeOf for Parcel {
    fn heap_size_of_children(&self) -> usize {
        0
    }
}

impl Parcel {
    /// Append object with a without signature into RLP stream
    pub fn rlp_append_unsigned_parcel(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&self.nonce);
        s.append(&self.fee);
        s.append(&self.network_id);
        s.append(&self.action);
    }

    /// The message hash of the parcel.
    pub fn hash(&self) -> H256 {
        let mut stream = RlpStream::new();
        self.rlp_append_unsigned_parcel(&mut stream);
        blake256(stream.as_raw())
    }

    pub fn iter_transactions<'a>(&'a self) -> Box<Iterator<Item = H256> + 'a> {
        match &self.action {
            Action::AssetTransactionGroup {
                transactions,
                ..
            } => Box::new(transactions.iter().map(|t| t.hash())),
            _ => Box::new(::std::iter::empty()),
        }
    }
}

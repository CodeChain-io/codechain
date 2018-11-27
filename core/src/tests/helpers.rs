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

use cstate::StateDB;
use primitives::{Bytes, H256, U256};
use rlp::{self, RlpStream};

use crate::header::Header;
use crate::parcel::SignedParcel;
use crate::scheme::Scheme;

pub fn create_test_block(header: &Header) -> Bytes {
    let mut rlp = RlpStream::new_list(2);
    rlp.append(header);
    rlp.append_raw(&rlp::EMPTY_LIST_RLP, 1);
    rlp.out()
}

#[allow(dead_code)]
pub fn create_test_block_with_data(header: &Header, parcels: &[SignedParcel], uncles: &[Header]) -> Bytes {
    let mut rlp = RlpStream::new_list(3);
    rlp.append(header);
    rlp.begin_list(parcels.len());
    for t in parcels {
        rlp.append_raw(&rlp::encode(t).into_vec(), 1);
    }
    rlp.append_list(&uncles);
    rlp.out()
}

pub fn get_good_dummy_block() -> Bytes {
    let (_, bytes) = get_good_dummy_block_hash();
    bytes
}

pub fn get_good_dummy_block_hash() -> (H256, Bytes) {
    let mut block_header = Header::new();
    let test_scheme = Scheme::new_test();
    block_header.set_score(U256::from(0x20000));
    block_header.set_timestamp(40);
    block_header.set_number(1);
    block_header.set_parent_hash(test_scheme.genesis_header().hash());

    (block_header.hash(), create_test_block(&block_header))
}

pub fn get_temp_state_db() -> StateDB {
    StateDB::new_with_memorydb(Vec::new())
}

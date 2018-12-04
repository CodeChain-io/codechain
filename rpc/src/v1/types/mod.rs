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

mod action;
mod block;
mod parcel;
mod transaction;
mod transaction_with_hash;
mod unsigned_parcel;
mod work;

use primitives::H256;

pub use self::action::{Action, ActionWithTxHash};
pub use self::block::Block;
pub use self::block::BlockNumberAndHash;
pub use self::parcel::Parcel;
pub use self::transaction::Transaction;
pub use self::transaction_with_hash::TransactionWithHash;
pub use self::unsigned_parcel::UnsignedParcel;
pub use self::work::Work;

#[derive(Debug, Serialize, Deserialize)]
pub struct FilterStatus {
    pub list: Vec<(::std::net::IpAddr, String)>,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendParcelResult {
    pub hash: H256,
    pub seq: u64,
}

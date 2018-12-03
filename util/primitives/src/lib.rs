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

extern crate ethcore_bytes as ebytes;
extern crate ethereum_types;

mod hash;

pub use crate::hash::{H128, H160, H256, H264, H512, H520};
pub use ebytes::Bytes;
pub use ethereum_types::{clean_0x, U128, U256};

pub mod bytes {
    pub use ebytes::ToPretty;
}

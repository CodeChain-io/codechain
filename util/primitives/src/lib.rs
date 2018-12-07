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
pub use ethereum_types::{clean_0x, U256};

pub mod bytes {
    pub use ebytes::ToPretty;
}

pub fn u256_from_u128(u: u128) -> U256 {
    let mut arr: [u64; 4] = [0, 0, 0, 0];
    arr[0] = (u & u128::from(std::u64::MAX)) as u64;
    arr[1] = (u >> 64) as u64;
    U256(arr)
}

#[cfg(test)]
mod tests {
    use ethereum_types::U128;

    use super::*;

    #[test]
    fn u128_zero() {
        assert_eq!(u256_from_u128(0), U128::zero().into());
    }

    #[test]
    fn u128_one() {
        assert_eq!(u256_from_u128(1), U128::one().into());
    }

    #[test]
    fn u64_max_plus_1() {
        let u128: U128 = U128::from(std::u64::MAX) + 1.into();
        assert_eq!(u256_from_u128(u128::from(std::u64::MAX) + 1), u128.into());
    }

    #[test]
    fn u128_max_minus_1() {
        let u128: U128 = U128::max_value() - 1.into();
        assert_eq!(u256_from_u128(std::u128::MAX - 1), u128.into());
    }

    #[test]
    fn u128_max() {
        assert_eq!(u256_from_u128(std::u128::MAX), U128::max_value().into());
    }
}

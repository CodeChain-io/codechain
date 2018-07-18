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

pub use ethereum_types::{H1024, H128, H160, H256, H264, H32, H512, H520, H64};

construct_hash!(H248, 31);

impl From<H248> for H256 {
    fn from(value: H248) -> H256 {
        let mut ret = H256::zero();
        ret.0[1..32].copy_from_slice(&value);
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h248_can_be_converted_to_h256() {
        let h248 = H248::random();
        let h256 = H256::from(h248);

        assert_eq!(0u8, h256[0]);
        assert_eq!(h248[0..31], h256[1..32]);
    }
}

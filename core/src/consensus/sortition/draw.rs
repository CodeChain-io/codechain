// Copyright 2019 Kodebox, Inc.
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

use rug::{integer::Order, Integer, Rational};

use super::binom_cdf::binomial_cdf_walk;

/// Draws leaders based on the vrf sortition algorithm. It divides the vrf_output by the
/// maximum value in the hash space and maps the result to a rational number.
///
/// # Arguments
/// * `voting_power`    - voting power of the player who draws leaders
/// * `total_power` - total power of the sortition
/// * `expectation` - expected eligible leaders in the system
/// * `vrf_output`  - vrf output hash the measure of eligibility
///
/// # Returns
/// * The number of winning draws in the lottery.

pub fn draw(voting_power: u64, total_power: u64, expectation: f64, vrf_output: &[u8]) -> u64 {
    let binomial_n = voting_power;
    let binomial_p = expectation / total_power as f64;

    let bitlen = 8 * (vrf_output.len()) as u32;
    let vrf_integer = Integer::from_digits(&vrf_output[..], Order::Msf);
    let vrf_max = (Integer::from(1) << bitlen) - 1;

    let lottery = Rational::from((vrf_integer, vrf_max)).to_f64();
    binomial_cdf_walk(binomial_p, binomial_n, lottery, voting_power)
}

#[cfg(test)]
mod lot_tests {
    use super::*;
    #[test]
    fn check_lot() {
        let voting_power = 4;
        let total_power = 16;
        let expectation = 8.0;
        assert_eq!(draw(voting_power, total_power, expectation, &[0x0f, 0x5c]), 0);
        assert_eq!(draw(voting_power, total_power, expectation, &[0x4c, 0xcc]), 1);
        assert_eq!(draw(voting_power, total_power, expectation, &[0xae, 0x13]), 2);
        assert_eq!(draw(voting_power, total_power, expectation, &[0xee, 0x13]), 3);
        assert_eq!(draw(voting_power, total_power, expectation, &[0xf3, 0x32]), 4);
    }
}

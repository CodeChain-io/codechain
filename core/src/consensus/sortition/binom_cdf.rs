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

use statrs::distribution::{Binomial, Univariate};

/// Determine where the lottery value is included walking along the
/// binomial cumulative distribution function.
///
/// # Arguments
///
/// * `p`   - probability parameter in the binomial distribution
/// * `n`   - trial parameter in the binomial distribution
/// * `lottery` - the target lottery value in the closed interval [0, 1]
/// * `voting_power` - determines the number of walks in cdf, where the maximum possible number of walks is n in the binomial distribution
///
/// # Returns
///
/// * The j value that includes the lottery value in the interval [cdf(j - 1), cdf(j)]

pub fn binomial_cdf_walk(p: f64, n: u64, lottery: f64, voting_power: u64) -> u64 {
    assert!(voting_power <= n);
    let dist = Binomial::new(p, n).expect("p must be in the closed interval [0.0, 1.0]");

    (0..voting_power)
        .find(|j| {
            let bound = dist.cdf(*j as f64);
            lottery <= bound
        })
        .unwrap_or(voting_power)
}

#[cfg(test)]
mod cdf_walk_tests {
    use super::*;
    #[test]
    fn check_cdf_walk_against_precalculated_values() {
        let p = 0.5;
        let n = 4;
        let voting_power = 4;
        assert_eq!(binomial_cdf_walk(p, n, 0.06, voting_power), 0);
        assert_eq!(binomial_cdf_walk(p, n, 0.3, voting_power), 1);
        assert_eq!(binomial_cdf_walk(p, n, 0.68, voting_power), 2);
        assert_eq!(binomial_cdf_walk(p, n, 0.93, voting_power), 3);
        assert_eq!(binomial_cdf_walk(p, n, 0.95, voting_power), 4);
    }
}

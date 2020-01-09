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

use ckey::Address;
use std::collections::hash_map;
use std::collections::HashMap;
use std::convert::TryFrom;

pub fn fee_distribute(total_min_fee: u64, stakes: &HashMap<Address, u64>) -> FeeDistributeIter {
    FeeDistributeIter {
        total_stakes: stakes.values().sum(),
        total_min_fee,
        remaining_fee: total_min_fee,
        stake_holdings: stakes.iter(),
    }
}

fn share(total_stakes: u64, stake: u64, total_min_fee: u64) -> u64 {
    assert!(total_stakes >= stake);
    u64::try_from((u128::from(total_min_fee) * u128::from(stake)) / u128::from(total_stakes)).unwrap()
}

pub struct FeeDistributeIter<'a> {
    total_stakes: u64,
    total_min_fee: u64,
    remaining_fee: u64,
    stake_holdings: hash_map::Iter<'a, Address, u64>,
}

impl<'a> FeeDistributeIter<'a> {
    pub fn remaining_fee(&self) -> u64 {
        self.remaining_fee
    }
}

impl<'a> Iterator for FeeDistributeIter<'a> {
    type Item = (&'a Address, u64);
    fn next(&mut self) -> Option<(&'a Address, u64)> {
        if let Some((stakeholder, stake)) = self.stake_holdings.next() {
            let share = share(self.total_stakes, *stake, self.total_min_fee);
            self.remaining_fee = self.remaining_fee.checked_sub(share).expect("Remaining fee shouldn't be depleted");
            Some((stakeholder, share))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribute_even() {
        let address1 = Address::random();
        let address2 = Address::random();
        let mut stakes = HashMap::new();
        stakes.insert(address1, 10);
        stakes.insert(address2, 10);

        let shares: HashMap<Address, u64> = fee_distribute(100, &stakes).map(|(k, v)| (*k, v)).collect();
        assert_eq!(shares, {
            let mut expected = HashMap::with_capacity(stakes.len());
            expected.insert(address1, 50);
            expected.insert(address2, 50);
            expected
        });
    }

    #[test]
    fn distribute_and_changes() {
        let addresses: Vec<_> = (0..51).map(|_| Address::random()).collect();
        let mut stakes = HashMap::with_capacity(addresses.len());
        for address in &addresses {
            stakes.insert(*address, 10);
        }

        let total = 100;
        let mut iter = fee_distribute(total, &stakes);
        let shares: HashMap<Address, u64> = (&mut iter).map(|(k, v)| (*k, v)).collect();

        let author_share = iter.remaining_fee();

        assert_eq!(49, author_share);
        assert_eq!(shares, {
            let mut expected = HashMap::with_capacity(addresses.len() + 1);
            for address in &addresses {
                expected.insert(*address, 1);
            }
            expected
        });
    }
}

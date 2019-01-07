// Copyright 2018-2019 Kodebox, Inc.
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

pub fn fee_distribute<'a>(author: &'a Address, fee: u64, stakes: &'a HashMap<Address, u64>) -> FeeDistributeIter<'a> {
    FeeDistributeIter {
        total_stakes: stakes.values().sum(),
        total_fee: fee,
        remaining_fee: fee,
        author,
        stake_holdings: stakes.iter(),
    }
}

pub struct FeeDistributeIter<'a> {
    total_stakes: u64,
    total_fee: u64,
    remaining_fee: u64,
    author: &'a Address,
    stake_holdings: hash_map::Iter<'a, Address, u64>,
}

impl<'a> Iterator for FeeDistributeIter<'a> {
    type Item = (&'a Address, u64);
    fn next(&mut self) -> Option<(&'a Address, u64)> {
        if let Some((stakeholder, stake)) = self.stake_holdings.next() {
            debug_assert!(self.total_stakes >= *stake);
            // promote u64 to u128 in order not to overflow while multiplication.
            let share = ((u128::from(self.total_fee) * u128::from(*stake)) / u128::from(self.total_stakes)) as u64;
            assert!(self.remaining_fee >= share, "Remaining fee shouldn't be depleted");
            self.remaining_fee -= share;
            Some((stakeholder, share))
        } else if self.remaining_fee > 0 {
            // author get remaining fees.
            let author_share = self.remaining_fee;
            self.remaining_fee = 0;
            Some((self.author, author_share))
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
        let author = Address::random();
        let address1 = Address::random();
        let address2 = Address::random();
        let mut stakes = HashMap::new();
        stakes.insert(address1, 10);
        stakes.insert(address2, 10);

        let shares: HashMap<Address, u64> = fee_distribute(&author, 100, &stakes).map(|(k, v)| (*k, v)).collect();
        assert_eq!(shares, {
            let mut expected = HashMap::with_capacity(stakes.len());
            expected.insert(address1, 50);
            expected.insert(address2, 50);
            expected
        });
    }

    #[test]
    fn distribute_and_changes() {
        let author = Address::random();
        let addresses: Vec<_> = (0..51).map(|_| Address::random()).collect();
        let mut stakes = HashMap::with_capacity(addresses.len());
        for address in &addresses {
            stakes.insert(*address, 10);
        }

        let shares: HashMap<Address, u64> = fee_distribute(&author, 100, &stakes).map(|(k, v)| (*k, v)).collect();

        assert_eq!(shares, {
            let mut expected = HashMap::with_capacity(addresses.len() + 1);
            expected.insert(author, 49);
            for address in &addresses {
                expected.insert(*address, 1);
            }
            expected
        });
    }
}

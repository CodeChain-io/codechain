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

extern crate rand;

pub use crate::random::new;
pub use rand::Rng;

#[cfg(not(test))]
mod random {
    use rand;
    pub fn new() -> rand::ThreadRng {
        rand::thread_rng()
    }
}
#[cfg(test)]
mod random {
    use rand::{self, SeedableRng};
    pub fn new() -> rand::XorShiftRng {
        rand::XorShiftRng::from_seed([0, 1, 2, 3])
    }
}

#[cfg(test)]
mod tests {
    use super::{random, Rng};

    #[test]
    fn return_deterministic_values_in_test_cfg() {
        let mut rng = random::new();
        let vs = rng.gen_iter::<u8>().take(6).collect::<Vec<u8>>();
        assert_eq!(vs, vec![3u8, 10, 24, 3, 24, 74]);
    }
}

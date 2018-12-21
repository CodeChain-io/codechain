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

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::{mem, thread};

use never::Never;
use rand::rngs::OsRng;
#[cfg(test)]
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

use crate::{Generator, KeyPair, SECP256K1};

pub struct Random;

#[cfg(test)]
thread_local! {
    static RNG: RefCell<XorShiftRng> = {
        let thread_id: [u8; 8] = unsafe { mem::transmute(thread::current().id()) };
        let mut seed: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3, 4, 5, 6, 7];
        seed[0..8].copy_from_slice(&thread_id);
        RefCell::new(XorShiftRng::from_seed(seed))
    };
}

impl Generator for Random {
    type Error = ::std::io::Error;

    #[cfg(not(test))]
    fn generate(&mut self) -> Result<KeyPair, Self::Error> {
        let mut rng = OsRng::new()?;
        match rng.generate() {
            Ok(pair) => Ok(pair),
            Err(never) => match never {}, // LLVM unreachable
        }
    }

    #[cfg(test)]
    fn generate(&mut self) -> Result<KeyPair, Self::Error> {
        RNG.with(|rng| {
            match rng.borrow_mut().generate() {
                Ok(pair) => Ok(pair),
                Err(never) => match never {}, // LLVM unreachable
            }
        })
    }
}

impl Generator for OsRng {
    type Error = Never;

    fn generate(&mut self) -> Result<KeyPair, Self::Error> {
        let (sec, publ) = SECP256K1.generate_keypair(self).expect("context always created with full capabilities; qed");

        Ok(KeyPair::from_keypair(sec, publ))
    }
}

impl Generator for XorShiftRng {
    type Error = Never;

    fn generate(&mut self) -> Result<KeyPair, <Self as Generator>::Error> {
        let (sec, publ) = SECP256K1.generate_keypair(self).expect("context always created with full capabilities; qed");

        Ok(KeyPair::from_keypair(sec, publ))
    }
}

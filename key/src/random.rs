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

use crate::{Generator, KeyPair, SECP256K1};
use rand::os::OsRng;

pub struct Random;

impl Generator for Random {
    type Error = ::std::io::Error;

    fn generate(&mut self) -> Result<KeyPair, Self::Error> {
        let mut rng = OsRng::new()?;
        match rng.generate() {
            Ok(pair) => Ok(pair),
            Err(void) => match void {}, // LLVM unreachable
        }
    }
}

impl Generator for OsRng {
    type Error = ::Void;

    fn generate(&mut self) -> Result<KeyPair, Self::Error> {
        let (sec, publ) = SECP256K1.generate_keypair(self).expect("context always created with full capabilities; qed");

        Ok(KeyPair::from_keypair(sec, publ))
    }
}

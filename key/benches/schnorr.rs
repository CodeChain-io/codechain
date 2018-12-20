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

#![feature(test)]

extern crate codechain_key as ckey;
extern crate test;

use ckey::{recover_schnorr, sign_schnorr, verify_schnorr, Generator, Message, Random};
use test::Bencher;

#[bench]
fn schnorr_sign(b: &mut Bencher) {
    b.iter(|| {
        let key_pair = Random.generate().unwrap();
        let message = Message::random();
        let _signature = sign_schnorr(key_pair.private(), &message).unwrap();
    });
}

#[bench]
fn schnorr_sign_and_verify(b: &mut Bencher) {
    b.iter(|| {
        let key_pair = Random.generate().unwrap();
        let message = Message::random();
        let signature = sign_schnorr(key_pair.private(), &message).unwrap();
        assert_eq!(Ok(true), verify_schnorr(key_pair.public(), &signature, &message));
    });
}

#[bench]
fn schnorr_sign_and_recover(b: &mut Bencher) {
    b.iter(|| {
        let key_pair = Random.generate().unwrap();
        let message = Message::random();
        let signature = sign_schnorr(key_pair.private(), &message).unwrap();
        assert_eq!(Ok(*key_pair.public()), recover_schnorr(&signature, &message));
    });
}

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

use ckey::{recover, sign, verify, Generator, Message, Random};
use test::Bencher;

#[bench]
fn ecdsa_sign(b: &mut Bencher) {
    b.iter(|| {
        let key_pair = Random.generate().unwrap();
        let message = Message::random();
        let _signature = sign(key_pair.private(), &message).unwrap();
    })
}

#[bench]
fn ecdsa_sign_and_verify(b: &mut Bencher) {
    b.iter(|| {
        let key_pair = Random.generate().unwrap();
        let message = Message::random();
        let signature = sign(key_pair.private(), &message).unwrap();
        assert_eq!(Ok(true), verify(key_pair.public(), &signature, &message));
    })
}

#[bench]
fn ecdsa_sign_and_recover(b: &mut Bencher) {
    b.iter(|| {
        let key_pair = Random.generate().unwrap();
        let message = Message::random();
        let signature = sign(key_pair.private(), &message).unwrap();
        assert_eq!(Ok(*key_pair.public()), recover(&signature, &message));
    });
}

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

extern crate codechain_crypto as ccrypto;
extern crate codechain_key as ckey;
extern crate primitives;
extern crate test;

use ccrypto::Blake;
use ckey::{recover, recover_schnorr, sign, sign_schnorr, verify, verify_schnorr, Generator, Message, Random};
use primitives::H160;
use test::Bencher;

#[bench]
fn pay_with_ecdsa(b: &mut Bencher) {
    // A transaction only has a signature.
    let key_pair = Random.generate().unwrap();
    let transaction = Message::random();
    let transaction_hash = Blake::blake(transaction);
    let signature = sign(key_pair.private(), &transaction_hash).unwrap();
    b.iter(|| {
        let transaction_hash = Blake::blake(transaction);
        let result = recover(&signature, &transaction_hash);
        assert_eq!(Ok(*key_pair.public()), result);
    });
}

#[bench]
fn transfer_with_ecdsa(b: &mut Bencher) {
    // Assuming 2-input transfer transaction.
    let key_pair_0 = Random.generate().unwrap();
    let key_pair_1 = Random.generate().unwrap();
    let key_pair_2 = Random.generate().unwrap();

    let transaction = Message::random();
    let transaction_hash = Blake::blake(transaction);
    let signature_tx = sign(key_pair_0.private(), &transaction_hash).unwrap();
    let signature_1 = sign(key_pair_1.private(), &transaction_hash).unwrap();
    let signature_2 = sign(key_pair_2.private(), &transaction_hash).unwrap();

    let lock_script_1 = Message::random();
    let lock_script_hash_1: H160 = Blake::blake(lock_script_1);
    let lock_script_2 = Message::random();
    let lock_script_hash_2: H160 = Blake::blake(lock_script_2);

    b.iter(|| {
        // Transaction verification
        let transaction_hash = Blake::blake(transaction);
        let result = recover(&signature_tx, &transaction_hash);
        assert_eq!(Ok(*key_pair_0.public()), result);

        // Input 1 verification
        // Lock script hash check
        assert_eq!(lock_script_hash_1, Blake::blake(lock_script_1));
        // Unfortunately, hash again because of partial hashing
        let transaction_hash_1 = Blake::blake(transaction);
        assert_eq!(Ok(true), verify(key_pair_1.public(), &signature_1, &transaction_hash_1));

        // Input 2 verification
        assert_eq!(lock_script_hash_2, Blake::blake(lock_script_2));
        let transaction_hash_2 = Blake::blake(transaction);
        assert_eq!(Ok(true), verify(key_pair_2.public(), &signature_2, &transaction_hash_2));
    });
}


#[bench]
fn pay_with_schnorr(b: &mut Bencher) {
    // A transaction only has a signature.
    let key_pair = Random.generate().unwrap();
    let transaction = Message::random();
    let transaction_hash = Blake::blake(transaction);
    let signature = sign_schnorr(key_pair.private(), &transaction_hash).unwrap();
    b.iter(|| {
        let transaction_hash = Blake::blake(transaction);
        let result = recover_schnorr(&signature, &transaction_hash);
        assert_eq!(Ok(*key_pair.public()), result);
    });
}

#[bench]
fn transfer_with_schnorr(b: &mut Bencher) {
    // Assuming 2-input transfer transaction.
    let key_pair_0 = Random.generate().unwrap();
    let key_pair_1 = Random.generate().unwrap();
    let key_pair_2 = Random.generate().unwrap();

    let transaction = Message::random();
    let transaction_hash = Blake::blake(transaction);
    let signature_tx = sign_schnorr(key_pair_0.private(), &transaction_hash).unwrap();
    let signature_1 = sign_schnorr(key_pair_1.private(), &transaction_hash).unwrap();
    let signature_2 = sign_schnorr(key_pair_2.private(), &transaction_hash).unwrap();

    let lock_script_1 = Message::random();
    let lock_script_hash_1: H160 = Blake::blake(lock_script_1);
    let lock_script_2 = Message::random();
    let lock_script_hash_2: H160 = Blake::blake(lock_script_2);

    b.iter(|| {
        // Transaction verification
        let transaction_hash = Blake::blake(transaction);
        let result = recover_schnorr(&signature_tx, &transaction_hash);
        assert_eq!(Ok(*key_pair_0.public()), result);

        // Input 1 verification
        // Lock script hash check
        assert_eq!(lock_script_hash_1, Blake::blake(lock_script_1));
        // Unfortunately, hash again because of partial hashing
        let transaction_hash_1 = Blake::blake(transaction);
        assert_eq!(Ok(true), verify_schnorr(key_pair_1.public(), &signature_1, &transaction_hash_1));

        // Input 2 verification
        assert_eq!(lock_script_hash_2, Blake::blake(lock_script_2));
        let transaction_hash_2 = Blake::blake(transaction);
        assert_eq!(Ok(true), verify_schnorr(key_pair_2.public(), &signature_2, &transaction_hash_2));
    });
}

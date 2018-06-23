// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

extern crate codechain_keystore as ckeystore;
extern crate codechain_types as ctypes;
extern crate rand;

mod util;

use ckeystore::accounts_dir::RootDiskDirectory;
use ckeystore::ckeys::{verify_schnorr_address, Generator, KeyPair, Random, Secret};
use ckeystore::{KeyStore, SimpleSecretStore};
use util::TransientDir;

#[test]
fn secret_store_create() {
    let dir = TransientDir::create().unwrap();
    let _ = KeyStore::open(Box::new(dir)).unwrap();
}

#[test]
#[should_panic]
fn secret_store_open_not_existing() {
    let dir = TransientDir::open();
    let _ = KeyStore::open(Box::new(dir)).unwrap();
}

fn random_secret() -> Secret {
    let keypair = Random.generate().unwrap();
    return **keypair.private()
}

#[test]
fn secret_store_create_account() {
    let dir = TransientDir::create().unwrap();
    let store = KeyStore::open(Box::new(dir)).unwrap();
    assert_eq!(store.accounts().unwrap().len(), 0);
    assert!(store.insert_account(random_secret(), "").is_ok());
    assert_eq!(store.accounts().unwrap().len(), 1);
    assert!(store.insert_account(random_secret(), "").is_ok());
    assert_eq!(store.accounts().unwrap().len(), 2);
}

#[test]
fn secret_store_sign() {
    let dir = TransientDir::create().unwrap();
    let store = KeyStore::open(Box::new(dir)).unwrap();
    assert!(store.insert_account(random_secret(), "").is_ok());
    let accounts = store.accounts().unwrap();
    assert_eq!(accounts.len(), 1);
    assert!(store.sign(&accounts[0], "", &Default::default()).is_ok());
    assert!(store.sign(&accounts[0], "1", &Default::default()).is_err());
}

#[test]
fn secret_store_change_password() {
    let dir = TransientDir::create().unwrap();
    let store = KeyStore::open(Box::new(dir)).unwrap();
    assert!(store.insert_account(random_secret(), "").is_ok());
    let accounts = store.accounts().unwrap();
    assert_eq!(accounts.len(), 1);
    assert!(store.sign(&accounts[0], "", &Default::default()).is_ok());
    assert!(store.change_password(&accounts[0], "", "1").is_ok());
    assert!(store.sign(&accounts[0], "", &Default::default()).is_err());
    assert!(store.sign(&accounts[0], "1", &Default::default()).is_ok());
}

#[test]
fn secret_store_remove_account() {
    let dir = TransientDir::create().unwrap();
    let store = KeyStore::open(Box::new(dir)).unwrap();
    assert!(store.insert_account(random_secret(), "").is_ok());
    let accounts = store.accounts().unwrap();
    assert_eq!(accounts.len(), 1);
    assert!(store.remove_account(&accounts[0], "").is_ok());
    assert_eq!(store.accounts().unwrap().len(), 0);
    assert!(store.remove_account(&accounts[0], "").is_err());
}

fn pat_path() -> &'static str {
    match ::std::fs::metadata("keystore") {
        Ok(_) => "keystore/tests/res/pat",
        Err(_) => "tests/res/pat",
    }
}

fn ciphertext_path() -> &'static str {
    match ::std::fs::metadata("keystore") {
        Ok(_) => "keystore/tests/res/ciphertext",
        Err(_) => "tests/res/ciphertext",
    }
}

#[test]
fn secret_store_load_pat_files() {
    let dir = RootDiskDirectory::at(pat_path());
    let store = KeyStore::open(Box::new(dir)).unwrap();
    assert_eq!(
        store.accounts().unwrap(),
        vec!["0x3fc74504d2b491d73079975e302279540bf6e44e".into(), "0x41178717678e402bdb663d98fe47669d93b29603".into()]
    );
}

#[test]
fn test_decrypting_files_with_short_ciphertext() {
    // 0x3fc74504d2b491d73079975e302279540bf6e44e
    let kp1 = KeyPair::from_private(
        "000081c29e8142bb6a81bef5a92bda7a8328a5c85bb2f9542e76f9b0f94fc018".parse().unwrap(),
    ).unwrap();
    // 0x41178717678e402bdb663d98fe47669d93b29603
    let kp2 = KeyPair::from_private(
        "00fa7b3db73dc7dfdf8c5fbdb796d741e4488628c41fc4febd9160a866ba0f35".parse().unwrap(),
    ).unwrap();
    let dir = RootDiskDirectory::at(ciphertext_path());
    let store = KeyStore::open(Box::new(dir)).unwrap();
    let accounts = store.accounts().unwrap();
    assert_eq!(
        accounts,
        vec!["0x3fc74504d2b491d73079975e302279540bf6e44e".into(), "0x41178717678e402bdb663d98fe47669d93b29603".into()]
    );

    let message = Default::default();

    let s1 = store.sign(&accounts[0], "password", &message).unwrap();
    let s2 = store.sign(&accounts[1], "password", &message).unwrap();
    assert!(verify_schnorr_address(&accounts[0], &s1, &message).unwrap());
    assert!(verify_schnorr_address(&kp1.address(), &s1, &message).unwrap());
    assert!(verify_schnorr_address(&kp2.address(), &s2, &message).unwrap());
}

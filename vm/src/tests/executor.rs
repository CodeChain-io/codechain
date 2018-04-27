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

use ccrypto::blake256;
use ckeys::{sign_ecdsa, KeyPair, Private};
use ctypes::H520;

use secp256k1::key::{SecretKey, MINUS_ONE_KEY, ONE_KEY};

use executor::{execute, Config, RuntimeError, ScriptResult};
use opcode::OpCode;

#[test]
fn simple_success() {
    assert_eq!(execute(&[OpCode::PushI(1)], Config::default()), Ok(ScriptResult::Unlocked));
}

#[test]
fn simple_failure() {
    assert_eq!(execute(&[OpCode::PushI(0)], Config::default()), Ok(ScriptResult::Fail));
}

#[test]
fn underflow() {
    assert_eq!(execute(&[OpCode::Pop], Config::default()), Err(RuntimeError::StackUnderflow));
}

#[test]
fn out_of_memory() {
    let config = Config {
        max_memory: 2,
    };
    assert_eq!(
        execute(&[OpCode::PushI(0), OpCode::PushI(1), OpCode::PushI(2)], config),
        Err(RuntimeError::OutOfMemory)
    );
}

#[test]
fn valid_pay_to_public_key() {
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256("codechain");
    let signature = H520::from(sign_ecdsa(keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![OpCode::PushB(signature)];
    let lock_script = vec![OpCode::PushB(pubkey), OpCode::ChkSig];

    assert_eq!(
        execute(&[&unlock_script[..], &lock_script[..]].concat(), Config::default()),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn invalid_pay_to_public_key() {
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256("codechain");
    let lock_script = vec![OpCode::PushB(pubkey), OpCode::ChkSig];

    let invalid_keypair = KeyPair::from_private(Private::from(SecretKey::from(MINUS_ONE_KEY))).unwrap();
    let invalid_signature = H520::from(sign_ecdsa(invalid_keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![OpCode::PushB(invalid_signature)];

    assert_eq!(execute(&[&unlock_script[..], &lock_script[..]].concat(), Config::default()), Ok(ScriptResult::Fail));
}

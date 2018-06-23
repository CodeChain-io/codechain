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

use ccrypto::{blake256, BLAKE_EMPTY, BLAKE_NULL_RLP};
use ckeys::{sign_schnorr, KeyPair, Private};
use ctypes::{H160, H256, H512};

use secp256k1::key::{SecretKey, MINUS_ONE_KEY, ONE_KEY};

use executor::{execute, Config, RuntimeError, ScriptResult};
use instruction::Instruction;

#[test]
fn simple_success() {
    assert_eq!(
        execute(&[Instruction::Push(1)], &[], &[], H256::default(), Config::default()),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn simple_failure() {
    assert_eq!(execute(&[Instruction::Push(0)], &[], &[], H256::default(), Config::default()), Ok(ScriptResult::Fail));
}

#[test]
fn simple_burn() {
    assert_eq!(execute(&[Instruction::Burn], &[], &[], H256::default(), Config::default()), Ok(ScriptResult::Burnt));
}

#[test]
fn underflow() {
    assert_eq!(
        execute(&[Instruction::Pop], &[], &[], H256::default(), Config::default()),
        Err(RuntimeError::StackUnderflow)
    );
}

#[test]
fn out_of_memory() {
    let config = Config {
        max_memory: 2,
    };
    assert_eq!(
        execute(&[Instruction::Push(0), Instruction::Push(1), Instruction::Push(2)], &[], &[], H256::default(), config),
        Err(RuntimeError::OutOfMemory)
    );
}

#[test]
fn valid_pay_to_public_key() {
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256("asdf");
    let signature = H512::from(sign_schnorr(keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![Instruction::PushB(signature)];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(execute(&unlock_script, &[], &lock_script, message, Config::default()), Ok(ScriptResult::Unlocked));
}

#[test]
fn invalid_pay_to_public_key() {
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256("asdf");
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    let invalid_keypair = KeyPair::from_private(Private::from(SecretKey::from(MINUS_ONE_KEY))).unwrap();
    let invalid_signature = H512::from(sign_schnorr(invalid_keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![Instruction::PushB(invalid_signature)];

    assert_eq!(execute(&unlock_script[..], &[], &lock_script, message, Config::default()), Ok(ScriptResult::Fail));
}

#[test]
fn conditional_burn() {
    let lock_script = vec![Instruction::Eq, Instruction::Dup, Instruction::Jnz(1), Instruction::Burn];
    assert_eq!(
        execute(&[Instruction::Push(0)], &[vec![0]], &lock_script, H256::default(), Config::default()),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(&[Instruction::Push(0)], &[vec![1]], &lock_script, H256::default(), Config::default()),
        Ok(ScriptResult::Burnt)
    );
}

#[test]
fn test_blake256() {
    let lock_script = vec![Instruction::Blake256, Instruction::Eq];
    assert_eq!(
        execute(&[], &[vec![], BLAKE_EMPTY.to_vec()], &lock_script, H256::default(), Config::default()),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(&[], &[vec![], BLAKE_NULL_RLP.to_vec()], &lock_script, H256::default(), Config::default()),
        Ok(ScriptResult::Fail)
    );
    assert_eq!(
        execute(&[], &[vec![0x80], BLAKE_NULL_RLP.to_vec()], &lock_script, H256::default(), Config::default()),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(&[], &[vec![0x80], BLAKE_EMPTY.to_vec()], &lock_script, H256::default(), Config::default()),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn test_ripemd160() {
    const RIPEMD160_EMPTY: H160 = H160([
        0x9c, 0x11, 0x85, 0xa5, 0xc5, 0xe9, 0xfc, 0x54, 0x61, 0x28, 0x08, 0x97, 0x7e, 0xe8, 0xf5, 0x48, 0xb2, 0x25,
        0x8d, 0x31,
    ]);
    const RIPEMD160_NULL_RLP: H160 = H160([
        0xb4, 0x36, 0x44, 0x1e, 0x6b, 0xb8, 0x82, 0xfe, 0x0a, 0x0f, 0xa0, 0x32, 0x0c, 0xb2, 0xd9, 0x7d, 0x96, 0xb4,
        0xd1, 0xbc,
    ]);
    let lock_script = vec![Instruction::Ripemd160, Instruction::Eq];
    assert_eq!(
        execute(&[], &[vec![], RIPEMD160_EMPTY.to_vec()], &lock_script, H256::default(), Config::default()),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(&[], &[vec![], RIPEMD160_NULL_RLP.to_vec()], &lock_script, H256::default(), Config::default()),
        Ok(ScriptResult::Fail)
    );
    assert_eq!(
        execute(&[], &[vec![0x80], RIPEMD160_NULL_RLP.to_vec()], &lock_script, H256::default(), Config::default()),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(&[], &[vec![0x80], RIPEMD160_EMPTY.to_vec()], &lock_script, H256::default(), Config::default()),
        Ok(ScriptResult::Fail)
    );
}

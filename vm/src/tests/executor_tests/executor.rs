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

use ccrypto::{BLAKE_EMPTY, BLAKE_NULL_RLP};
use ckey::NetworkId;
use ctypes::transaction::{AssetOutPoint, Transaction};
use primitives::{H160, H256};

use executor::{execute, Config, RuntimeError, ScriptResult};
use instruction::Instruction;

#[test]
fn simple_success() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    assert_eq!(
        execute(&[], &[], &[Instruction::Push(1)], &transaction, Config::default(), &outpoint, false),
        Ok(ScriptResult::Unlocked)
    );

    assert_eq!(
        execute(&[], &[], &[Instruction::Success], &transaction, Config::default(), &outpoint, false),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn simple_failure() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    assert_eq!(
        execute(&[Instruction::Push(0)], &[], &[], &transaction, Config::default(), &outpoint, false),
        Ok(ScriptResult::Fail)
    );
    assert_eq!(
        execute(&[], &[], &[Instruction::Fail], &transaction, Config::default(), &outpoint, false),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn simple_burn() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    assert_eq!(
        execute(&[], &[], &[Instruction::Burn], &transaction, Config::default(), &outpoint, false),
        Ok(ScriptResult::Burnt)
    );
}

#[test]
fn underflow() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    assert_eq!(
        execute(&[], &[], &[Instruction::Pop], &transaction, Config::default(), &outpoint, false),
        Err(RuntimeError::StackUnderflow)
    );
}

#[test]
fn out_of_memory() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    let config = Config {
        max_memory: 2,
    };
    assert_eq!(
        execute(
            &[Instruction::Push(0), Instruction::Push(1), Instruction::Push(2)],
            &[],
            &[],
            &transaction,
            config,
            &outpoint,
            false
        ),
        Err(RuntimeError::OutOfMemory)
    );
}

#[test]
fn invalid_unlock_script() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    assert_eq!(
        execute(&[Instruction::Nop], &[], &[], &transaction, Config::default(), &outpoint, false),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn conditional_burn() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    let lock_script = vec![Instruction::Eq, Instruction::Dup, Instruction::Jnz(1), Instruction::Burn];
    assert_eq!(
        execute(&[Instruction::Push(0)], &[vec![0]], &lock_script, &transaction, Config::default(), &outpoint, false),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(&[Instruction::Push(0)], &[vec![1]], &lock_script, &transaction, Config::default(), &outpoint, false),
        Ok(ScriptResult::Burnt)
    );
}

#[test]
fn _blake256() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    let lock_script = vec![Instruction::Blake256, Instruction::Eq];
    assert_eq!(
        execute(&[], &[vec![], BLAKE_EMPTY.to_vec()], &lock_script, &transaction, Config::default(), &outpoint, false),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![], BLAKE_NULL_RLP.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Fail)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![0x80], BLAKE_NULL_RLP.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![0x80], BLAKE_EMPTY.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn _ripemd160() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
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
        execute(
            &[],
            &[vec![], RIPEMD160_EMPTY.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![], RIPEMD160_NULL_RLP.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Fail)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![0x80], RIPEMD160_NULL_RLP.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![0x80], RIPEMD160_EMPTY.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn _sha256() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    const SHA256_EMPTY: H256 = H256([
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24, 0x27, 0xae,
        0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
    ]);
    const SHA256_NULL_RLP: H256 = H256([
        0x76, 0xbe, 0x8b, 0x52, 0x8d, 0x00, 0x75, 0xf7, 0xaa, 0xe9, 0x8d, 0x6f, 0xa5, 0x7a, 0x6d, 0x3c, 0x83, 0xae,
        0x48, 0x0a, 0x84, 0x69, 0xe6, 0x68, 0xd7, 0xb0, 0xaf, 0x96, 0x89, 0x95, 0xac, 0x71,
    ]);
    let lock_script = vec![Instruction::Sha256, Instruction::Eq];
    assert_eq!(
        execute(&[], &[vec![], SHA256_EMPTY.to_vec()], &lock_script, &transaction, Config::default(), &outpoint, false),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![], SHA256_NULL_RLP.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Fail)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![0x80], SHA256_NULL_RLP.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![0x80], SHA256_EMPTY.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn _keccak256() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    const KECCAK256_EMPTY: H256 = H256([
        0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03, 0xc0, 0xe5, 0x00,
        0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85, 0xa4, 0x70,
    ]);
    const KECCAK256_NULL_RLP: H256 = H256([
        0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e, 0x5b, 0x48,
        0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
    ]);
    let lock_script = vec![Instruction::Keccak256, Instruction::Eq];
    assert_eq!(
        execute(
            &[],
            &[vec![], KECCAK256_EMPTY.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![], KECCAK256_NULL_RLP.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Fail)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![0x80], KECCAK256_NULL_RLP.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Unlocked)
    );
    assert_eq!(
        execute(
            &[],
            &[vec![0x80], KECCAK256_EMPTY.to_vec()],
            &lock_script,
            &transaction,
            Config::default(),
            &outpoint,
            false
        ),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn copy_stack_underflow() {
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        nonce: 0,
    };
    let outpoint = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    assert_eq!(
        execute(&[], &[], &[Instruction::Copy(1)], &transaction, Config::default(), &outpoint, false),
        Err(RuntimeError::StackUnderflow)
    );
}

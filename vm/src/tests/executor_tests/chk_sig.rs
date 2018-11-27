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

use ccrypto::{blake128, blake256_with_key};
use ckey::{sign, KeyPair, NetworkId, Private, Signature};
use ctypes::transaction::{AssetOutPoint, AssetTransferInput, AssetTransferOutput, Transaction};
use primitives::{H160, H256};
use rlp::Encodable;

use secp256k1::key::{SecretKey, MINUS_ONE_KEY, ONE_KEY};

use crate::executor::{execute, Config, ScriptResult};
use crate::instruction::Instruction;

use super::executor::get_test_client;

#[test]
fn valid_pay_to_public_key() {
    let client = get_test_client();
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    let input = AssetTransferInput {
        prev_out: AssetOutPoint {
            transaction_hash: H256::default(),
            index: 0,
            asset_type: H256::default(),
            amount: 0,
        },
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature = Signature::from(sign(keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b11 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &input, false, &client),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn invalid_pay_to_public_key() {
    let client = get_test_client();
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    let input = AssetTransferInput {
        prev_out: AssetOutPoint {
            transaction_hash: H256::default(),
            index: 0,
            asset_type: H256::default(),
            amount: 0,
        },
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );

    let invalid_keypair = KeyPair::from_private(Private::from(SecretKey::from(MINUS_ONE_KEY))).unwrap();
    let invalid_signature = Signature::from(sign(invalid_keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![Instruction::PushB(invalid_signature), Instruction::PushB(vec![0b11 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script[..], &[], &lock_script, &transaction, Config::default(), &input, false, &client),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn sign_all_input_all_output() {
    let client = get_test_client();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0.clone(),
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make input indexed 1
    let out1 = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 1,
        asset_type: H256::default(),
        amount: 1,
    };
    let input1 = AssetTransferInput {
        prev_out: out1,
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make output indexed 0
    let output0 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H256::default(),
        amount: 0,
    };
    // Make output indexed 1
    let output1 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H256::default(),
        amount: 1,
    };
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone(), input1.clone()],
        outputs: vec![output0.clone(), output1.clone()],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone(), input1],
            outputs: vec![output0, output1],
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );

    let signature = Signature::from(sign(keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b11 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &input0, false, &client),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn sign_single_input_all_output() {
    let client = get_test_client();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0.clone(),
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make input indexed 1
    let out1 = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 1,
        asset_type: H256::default(),
        amount: 1,
    };
    let input1 = AssetTransferInput {
        prev_out: out1,
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make output indexed 0
    let output0 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H256::default(),
        amount: 0,
    };
    // Make output indexed 1
    let output1 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H256::default(),
        amount: 1,
    };
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone(), input1.clone()],
        outputs: vec![output0.clone(), output1.clone()],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone()],
            outputs: vec![output0, output1],
        }
        .rlp_bytes(),
        &blake128(&[0b10 as u8]),
    );
    let signature = Signature::from(sign(keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b10 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &input0, false, &client),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn sign_all_input_partial_output() {
    let client = get_test_client();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0.clone(),
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make input indexed 1
    let out1 = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 1,
        asset_type: H256::default(),
        amount: 1,
    };
    let input1 = AssetTransferInput {
        prev_out: out1,
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make output indexed 0
    let output0 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H256::default(),
        amount: 0,
    };
    // Make output indexed 1
    let output1 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H256::default(),
        amount: 1,
    };
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone(), input1.clone()],
        outputs: vec![output0.clone(), output1.clone()],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone(), input1],
            outputs: vec![output0],
        }
        .rlp_bytes(),
        &blake128(&[0b1, 0b00000101 as u8]),
    );
    let signature = Signature::from(sign(keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b1, 0b00000101 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &input0, false, &client),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn sign_single_input_partial_output() {
    let client = get_test_client();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0.clone(),
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make input indexed 1
    let out1 = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 1,
        asset_type: H256::default(),
        amount: 1,
    };
    let input1 = AssetTransferInput {
        prev_out: out1,
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make output indexed 0
    let output0 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H256::default(),
        amount: 0,
    };
    // Make output indexed 1
    let output1 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H256::default(),
        amount: 1,
    };
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone(), input1.clone()],
        outputs: vec![output0.clone(), output1.clone()],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone()],
            outputs: vec![output0],
        }
        .rlp_bytes(),
        &blake128(&[0b1, 0b00000100 as u8]),
    );
    let signature = Signature::from(sign(keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b1, 0b00000100 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &input0, false, &client),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn distinguish_sign_single_input_with_sign_all() {
    let client = get_test_client();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0.clone(),
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make output indexed 0
    let output0 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H256::default(),
        amount: 0,
    };
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone()],
        outputs: vec![output0.clone()],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone()],
            outputs: vec![output0],
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature = Signature::from(sign(keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b10 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &input0, false, &client),
        Ok(ScriptResult::Fail)
    );
}


#[test]
fn distinguish_sign_single_output_with_sign_all() {
    let client = get_test_client();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        transaction_hash: H256::default(),
        index: 0,
        asset_type: H256::default(),
        amount: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0.clone(),
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make output indexed 0
    let output0 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H256::default(),
        amount: 0,
    };
    let transaction = Transaction::AssetTransfer {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone()],
        outputs: vec![output0.clone()],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(SecretKey::from(ONE_KEY))).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone()],
            outputs: vec![output0],
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature = Signature::from(sign(keypair.private(), &message).unwrap()).to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b1, 0b00000101 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &input0, false, &client),
        Ok(ScriptResult::Fail)
    );
}

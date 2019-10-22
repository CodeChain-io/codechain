// Copyright 2018-2019 Kodebox, Inc.
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

extern crate codechain_crypto as ccrypto;
extern crate codechain_key as ckey;
extern crate codechain_types as ctypes;
extern crate codechain_vm as cvm;
extern crate primitives;
extern crate rlp;
extern crate secp256k1;

mod common;

use ccrypto::{blake128, blake256_with_key};
use ckey::{sign, KeyPair, NetworkId, Private};
use ctypes::transaction::{AssetOutPoint, AssetTransferInput, AssetTransferOutput, ShardTransaction};
use primitives::H160;
use rlp::Encodable;
use secp256k1::key::{MINUS_ONE_KEY, ONE_KEY};

use cvm::Instruction;
use cvm::{execute, ScriptResult, VMConfig};

use common::TestClient;

#[test]
fn valid_pay_to_public_key() {
    let client = TestClient::default();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    let input = AssetTransferInput {
        prev_out: AssetOutPoint {
            tracker: Default::default(),
            index: 0,
            asset_type: H160::default(),
            shard_id: 0,
            quantity: 0,
        },
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    let keypair = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature = sign(keypair.private(), &message).unwrap().to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b11 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, VMConfig::default(), &input, false, &client, 0, 0),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn invalid_pay_to_public_key() {
    let client = TestClient::default();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    let input = AssetTransferInput {
        prev_out: AssetOutPoint {
            tracker: Default::default(),
            index: 0,
            asset_type: H160::default(),
            shard_id: 0,
            quantity: 0,
        },
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    let keypair = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );

    let invalid_keypair = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let invalid_signature = sign(invalid_keypair.private(), &message).unwrap().to_vec();
    let unlock_script = vec![Instruction::PushB(invalid_signature), Instruction::PushB(vec![0b11 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script[..], &[], &lock_script, &transaction, VMConfig::default(), &input, false, &client, 0, 0),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn sign_all_input_all_output() {
    let client = TestClient::default();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        tracker: Default::default(),
        index: 0,
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0,
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make input indexed 1
    let out1 = AssetOutPoint {
        tracker: Default::default(),
        index: 1,
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 1,
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
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    // Make output indexed 1
    let output1 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 1,
    };
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone(), input1.clone()],
        outputs: vec![output0.clone(), output1.clone()],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone(), input1],
            outputs: vec![output0, output1],
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );

    let signature = sign(keypair.private(), &message).unwrap().to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b11 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, VMConfig::default(), &input0, false, &client, 0, 0),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn sign_single_input_all_output() {
    let client = TestClient::default();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        tracker: Default::default(),
        index: 0,
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0,
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make input indexed 1
    let out1 = AssetOutPoint {
        tracker: Default::default(),
        index: 1,
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 1,
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
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    // Make output indexed 1
    let output1 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 1,
    };
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone(), input1],
        outputs: vec![output0.clone(), output1.clone()],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone()],
            outputs: vec![output0, output1],
        }
        .rlp_bytes(),
        &blake128(&[0b10 as u8]),
    );
    let signature = sign(keypair.private(), &message).unwrap().to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b10 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, VMConfig::default(), &input0, false, &client, 0, 0),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn sign_all_input_partial_output() {
    let client = TestClient::default();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        tracker: Default::default(),
        index: 0,
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0,
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make input indexed 1
    let out1 = AssetOutPoint {
        tracker: Default::default(),
        index: 1,
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 1,
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
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    // Make output indexed 1
    let output1 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 1,
    };
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone(), input1.clone()],
        outputs: vec![output0.clone(), output1],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone(), input1],
            outputs: vec![output0],
        }
        .rlp_bytes(),
        &blake128(&[0b1, 0b0000_0101 as u8]),
    );
    let signature = sign(keypair.private(), &message).unwrap().to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b1, 0b0000_0101 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, VMConfig::default(), &input0, false, &client, 0, 0),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn sign_single_input_partial_output() {
    let client = TestClient::default();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        tracker: Default::default(),
        index: 0,
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0,
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make input indexed 1
    let out1 = AssetOutPoint {
        tracker: Default::default(),
        index: 1,
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 1,
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
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    // Make output indexed 1
    let output1 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 1,
    };
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone(), input1],
        outputs: vec![output0.clone(), output1],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone()],
            outputs: vec![output0],
        }
        .rlp_bytes(),
        &blake128(&[0b1, 0b0000_0100 as u8]),
    );
    let signature = sign(keypair.private(), &message).unwrap().to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b1, 0b0000_0100 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, VMConfig::default(), &input0, false, &client, 0, 0),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn distinguish_sign_single_input_with_sign_all() {
    let client = TestClient::default();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        tracker: Default::default(),
        index: 0,
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0,
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make output indexed 0
    let output0 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone()],
        outputs: vec![output0.clone()],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone()],
            outputs: vec![output0],
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature = sign(keypair.private(), &message).unwrap().to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b10 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, VMConfig::default(), &input0, false, &client, 0, 0),
        Ok(ScriptResult::Fail)
    );
}


#[test]
fn distinguish_sign_single_output_with_sign_all() {
    let client = TestClient::default();
    // Make input indexed 0
    let out0 = AssetOutPoint {
        tracker: Default::default(),
        index: 0,
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    let input0 = AssetTransferInput {
        prev_out: out0,
        timelock: None,
        lock_script: Vec::new(),
        unlock_script: Vec::new(),
    };
    // Make output indexed 0
    let output0 = AssetTransferOutput {
        lock_script_hash: H160::default(),
        parameters: Vec::new(),
        asset_type: H160::default(),
        shard_id: 0,
        quantity: 0,
    };
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: vec![input0.clone()],
        outputs: vec![output0.clone()],
    };

    // Execute sciprt in input0
    let keypair = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let pubkey = <&[u8]>::from(keypair.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: vec![input0.clone()],
            outputs: vec![output0],
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature = sign(keypair.private(), &message).unwrap().to_vec();
    let unlock_script = vec![Instruction::PushB(signature), Instruction::PushB(vec![0b1, 0b0000_0101 as u8])];
    let lock_script = vec![Instruction::PushB(pubkey), Instruction::ChkSig];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, VMConfig::default(), &input0, false, &client, 0, 0),
        Ok(ScriptResult::Fail)
    );
}

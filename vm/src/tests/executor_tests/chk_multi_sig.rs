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
use ckey::{sign, KeyPair, NetworkId, Private};
use ctypes::transaction::{AssetOutPoint, AssetTransferInput, ShardTransaction};
use primitives::H256;
use rlp::Encodable;

use secp256k1::key::{MINUS_ONE_KEY, ONE_KEY, TWO_KEY};

use crate::executor::{execute, Config, RuntimeError, ScriptResult};
use crate::instruction::Instruction;

use super::executor::get_test_client;

#[test]
fn valid_multi_sig_0_of_2() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();

    let unlock_script = vec![Instruction::PushB(vec![0b11 as u8])];
    let lock_script = vec![
        Instruction::PushB(vec![0]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(vec![2]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Err(RuntimeError::InvalidSigCount)
    );
}

#[test]
fn valid_multi_sig_1_of_2() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message).unwrap().to_vec();

    let unlock_script = vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature1)];
    let lock_script = vec![
        Instruction::PushB(vec![1]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(vec![2]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn valid_multi_sig_2_of_2() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message).unwrap().to_vec();
    let signature2 = sign(keypair2.private(), &message).unwrap().to_vec();

    let unlock_script =
        vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature1), Instruction::PushB(signature2)];
    let lock_script = vec![
        Instruction::PushB(vec![2]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(vec![2]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn valid_multi_sig_2_of_3_110() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let keypair3 = KeyPair::from_private(Private::from(TWO_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let pubkey3 = <&[u8]>::from(keypair3.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message).unwrap().to_vec();
    let signature2 = sign(keypair2.private(), &message).unwrap().to_vec();

    let unlock_script =
        vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature1), Instruction::PushB(signature2)];
    let lock_script = vec![
        Instruction::PushB(vec![2]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(pubkey3),
        Instruction::PushB(vec![3]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn valid_multi_sig_2_of_3_101() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let keypair3 = KeyPair::from_private(Private::from(TWO_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let pubkey3 = <&[u8]>::from(keypair3.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message).unwrap().to_vec();
    let signature3 = sign(keypair3.private(), &message).unwrap().to_vec();

    let unlock_script =
        vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature1), Instruction::PushB(signature3)];
    let lock_script = vec![
        Instruction::PushB(vec![2]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(pubkey3),
        Instruction::PushB(vec![3]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn valid_multi_sig_2_of_3_011() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let keypair3 = KeyPair::from_private(Private::from(TWO_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let pubkey3 = <&[u8]>::from(keypair3.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature2 = sign(keypair2.private(), &message).unwrap().to_vec();
    let signature3 = sign(keypair3.private(), &message).unwrap().to_vec();

    let unlock_script =
        vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature2), Instruction::PushB(signature3)];
    let lock_script = vec![
        Instruction::PushB(vec![2]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(pubkey3),
        Instruction::PushB(vec![3]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Ok(ScriptResult::Unlocked)
    );
}

#[test]
fn invalid_multi_sig_1_of_2() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: "aa".into(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message).unwrap().to_vec();

    let unlock_script = vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature1)];
    let lock_script = vec![
        Instruction::PushB(vec![1]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(vec![2]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Ok(ScriptResult::Fail)
    );
}


#[test]
fn invalid_multi_sig_2_of_2() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: "aa".into(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message).unwrap().to_vec();
    let signature2 = sign(keypair2.private(), &message).unwrap().to_vec();

    let unlock_script =
        vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature1), Instruction::PushB(signature2)];
    let lock_script = vec![
        Instruction::PushB(vec![2]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(vec![2]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn invalid_multi_sig_2_of_2_with_1_invalid_sig() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let message1 = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let message2 = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: "aa".into(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message1).unwrap().to_vec();
    let signature2 = sign(keypair2.private(), &message2).unwrap().to_vec();

    let unlock_script =
        vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature1), Instruction::PushB(signature2)];
    let lock_script = vec![
        Instruction::PushB(vec![2]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(vec![2]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn invalid_multi_sig_2_of_2_with_changed_order_sig() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message).unwrap().to_vec();
    let signature2 = sign(keypair2.private(), &message).unwrap().to_vec();

    let unlock_script =
        vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature2), Instruction::PushB(signature1)];
    let lock_script = vec![
        Instruction::PushB(vec![2]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(vec![2]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Ok(ScriptResult::Fail)
    );
}

#[test]
fn invalid_multi_sig_with_less_sig_than_m() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message).unwrap().to_vec();

    let unlock_script = vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature1)];
    let lock_script = vec![
        Instruction::PushB(vec![2]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(vec![2]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn invalid_multi_sig_with_more_sig_than_m() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let keypair2 = KeyPair::from_private(Private::from(MINUS_ONE_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let pubkey2 = <&[u8]>::from(keypair2.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message).unwrap().to_vec();
    let signature2 = sign(keypair2.private(), &message).unwrap().to_vec();

    let unlock_script =
        vec![Instruction::PushB(vec![0b11 as u8]), Instruction::PushB(signature1), Instruction::PushB(signature2)];
    let lock_script = vec![
        Instruction::PushB(vec![1]),
        Instruction::PushB(pubkey1),
        Instruction::PushB(pubkey2),
        Instruction::PushB(vec![2]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Err(RuntimeError::InvalidFilter)
    );
}

#[test]
fn invalid_multi_sig_with_too_many_arg() {
    let client = get_test_client();
    let transaction = ShardTransaction::TransferAsset {
        network_id: NetworkId::default(),
        burns: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        orders: Vec::new(),
    };
    let outpoint = AssetTransferInput {
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
    let keypair1 = KeyPair::from_private(Private::from(ONE_KEY)).unwrap();
    let pubkey1 = <&[u8]>::from(keypair1.public()).to_vec();
    let message = blake256_with_key(
        &ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
        .rlp_bytes(),
        &blake128(&[0b11 as u8]),
    );
    let signature1 = sign(keypair1.private(), &message).unwrap().to_vec();

    let unlock_script = vec![
        Instruction::PushB(vec![0b11 as u8]),
        Instruction::PushB(signature1.clone()),
        Instruction::PushB(signature1.clone()),
        Instruction::PushB(signature1.clone()),
        Instruction::PushB(signature1.clone()),
        Instruction::PushB(signature1.clone()),
        Instruction::PushB(signature1.clone()),
        Instruction::PushB(signature1),
    ];
    let lock_script = vec![
        Instruction::PushB(vec![7]),
        Instruction::PushB(pubkey1.clone()),
        Instruction::PushB(pubkey1.clone()),
        Instruction::PushB(pubkey1.clone()),
        Instruction::PushB(pubkey1.clone()),
        Instruction::PushB(pubkey1.clone()),
        Instruction::PushB(pubkey1.clone()),
        Instruction::PushB(pubkey1),
        Instruction::PushB(vec![7]),
        Instruction::ChkMultiSig,
    ];

    assert_eq!(
        execute(&unlock_script, &[], &lock_script, &transaction, Config::default(), &outpoint, false, &client),
        Err(RuntimeError::InvalidSigCount)
    );
}

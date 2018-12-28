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

use byteorder::{BigEndian, ByteOrder};

use ccrypto::{blake256, keccak256, ripemd160, sha256, Blake};
use ckey::{verify, Public, Signature, SIGNATURE_LENGTH};
use ctypes::transaction::{AssetTransferInput, HashingError, PartialHashing};
use ctypes::util::tag::Tag;

use primitives::{H160, H256};


use crate::instruction::{has_expensive_opcodes, is_valid_unlock_script, Instruction};

const DEFAULT_MAX_MEMORY: usize = 1024;

const TIMELOCK_TYPE_BLOCK: u8 = 0x01;
const TIMELOCK_TYPE_BLOCK_AGE: u8 = 0x02;
const TIMELOCK_TYPE_TIME: u8 = 0x03;
const TIMELOCK_TYPE_TIME_AGE: u8 = 0x04;

pub struct Config {
    pub max_memory: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_memory: DEFAULT_MAX_MEMORY,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ScriptResult {
    Fail,
    Unlocked,
    Burnt,
}

#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    OutOfMemory,
    IndexOutOfBound,
    StackUnderflow,
    TypeMismatch,
    InvalidFilter,
    InvalidSigCount,
    InvalidTimelockType,
}

impl From<HashingError> for RuntimeError {
    fn from(error: HashingError) -> Self {
        match error {
            HashingError::InvalidFilter => RuntimeError::InvalidFilter,
        }
    }
}

#[derive(Clone)]
struct Item(Vec<u8>);

impl Item {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn assert_len(self, len: usize) -> Result<Self, RuntimeError> {
        if self.len() == len {
            Ok(self)
        } else {
            Err(RuntimeError::TypeMismatch)
        }
    }
}

impl AsRef<[u8]> for Item {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<bool> for Item {
    fn from(val: bool) -> Item {
        if val {
            Item(vec![1])
        } else {
            Item(vec![])
        }
    }
}

impl From<Item> for bool {
    fn from(item: Item) -> Self {
        item.as_ref().iter().any(|b| b != &0)
    }
}

struct Stack {
    stack: Vec<Item>,
    memory_usage: usize,
    config: Config,
}

impl Stack {
    fn new(config: Config) -> Self {
        Self {
            stack: Vec::new(),
            memory_usage: 0,
            config,
        }
    }

    /// Returns true if value is successfully pushed
    fn push(&mut self, val: Item) -> Result<(), RuntimeError> {
        if self.memory_usage + val.len() > self.config.max_memory {
            Err(RuntimeError::OutOfMemory)
        } else {
            self.memory_usage += val.len();
            self.stack.push(val);
            Ok(())
        }
    }

    fn pop(&mut self) -> Result<Item, RuntimeError> {
        let item = self.stack.pop();
        self.memory_usage -= item.as_ref().map_or(0, |i| i.len());
        item.ok_or(RuntimeError::StackUnderflow)
    }

    fn len(&self) -> usize {
        self.stack.len()
    }

    fn get(&self, index: usize) -> Result<Item, RuntimeError> {
        self.stack.get(index).cloned().ok_or(RuntimeError::IndexOutOfBound)
    }

    fn remove(&mut self, index: usize) -> Result<Item, RuntimeError> {
        if index < self.stack.len() {
            let item = self.stack.remove(index);
            self.memory_usage -= item.len();
            Ok(item)
        } else {
            Err(RuntimeError::IndexOutOfBound)
        }
    }
}

pub fn execute<C>(
    unlock: &[Instruction],
    params: &[Vec<u8>],
    lock: &[Instruction],
    tx: &PartialHashing,
    config: Config,
    cur: &AssetTransferInput,
    burn: bool,
    client: &C,
) -> Result<ScriptResult, RuntimeError>
where
    C: ChainTimeInfo, {
    // FIXME: don't merge scripts

    if !is_valid_unlock_script(unlock) {
        return Ok(ScriptResult::Fail)
    }

    if has_expensive_opcodes(unlock) {
        return Ok(ScriptResult::Fail)
    }

    let param_scripts: Vec<_> = params.iter().map(|p| Instruction::PushB(p.clone())).rev().collect();
    let script = [unlock, &param_scripts, lock].concat();

    let mut stack = Stack::new(config);
    let mut pc = 0;
    while pc < script.len() {
        match &script[pc] {
            Instruction::Nop => {}
            Instruction::Burn => return Ok(ScriptResult::Burnt),
            Instruction::Success => return Ok(ScriptResult::Unlocked),
            Instruction::Fail => return Ok(ScriptResult::Fail),
            Instruction::Not => {
                let value: bool = stack.pop()?.into();
                stack.push(Item::from(!value))?;
            }
            Instruction::Eq => {
                let first = stack.pop()?;
                let second = stack.pop()?;
                stack.push(Item::from(first.as_ref() == second.as_ref()))?;
            }
            Instruction::Jmp(val) => {
                pc += *val as usize;
            }
            Instruction::Jnz(val) => {
                if stack.pop()?.into() {
                    pc += *val as usize;
                }
            }
            Instruction::Jz(val) => {
                let condition: bool = stack.pop()?.into();
                if !condition {
                    pc += *val as usize;
                }
            }
            Instruction::Push(val) => stack.push(Item(vec![*val]))?,
            Instruction::Pop => {
                stack.pop()?;
            }
            Instruction::PushB(blob) => stack.push(Item(blob.clone()))?,
            Instruction::Dup => {
                let top = stack.pop()?;
                stack.push(top.clone())?;
                stack.push(top)?;
            }
            Instruction::Swap => {
                let first = stack.pop()?;
                let second = stack.pop()?;
                stack.push(first)?;
                stack.push(second)?;
            }
            Instruction::Copy(index) => {
                if stack.len() <= *index as usize {
                    return Err(RuntimeError::StackUnderflow)
                }
                let item = stack.get((stack.len() - 1) - *index as usize)?;
                stack.push(item)?
            }
            Instruction::Drop(index) => {
                stack.remove(*index as usize)?;
            }
            Instruction::ChkSig => {
                let pubkey = Public::from_slice(stack.pop()?.assert_len(64)?.as_ref());
                let tag = Tag::try_new(stack.pop()?.as_ref().to_vec())?;
                let tx_hash = tx.hash_partially(tag, cur, burn)?;
                let signature = Signature::from(stack.pop()?.assert_len(SIGNATURE_LENGTH)?.as_ref());
                ::clogger::metric_logger.increase("vm::checksig");
                let result = match verify(&pubkey, &signature, &tx_hash) {
                    Ok(true) => 1,
                    _ => 0,
                };
                stack.push(Item(vec![result]))?;
            }
            Instruction::ChkMultiSig => {
                // Get n pubkey. If there are more than six pubkeys, return error.
                let n = stack.pop()?.assert_len(1)?.as_ref()[0] as usize;

                let mut pubkey: Vec<Public> = Vec::with_capacity(n);
                for _ in 0..n {
                    pubkey.push(Public::from_slice(stack.pop()?.assert_len(64)?.as_ref()));
                }

                // Get m signature. If signatures are more than pubkeys, return error.
                let m = stack.pop()?.assert_len(1)?.as_ref()[0] as usize;
                if m > n || m == 0 || m > 6 {
                    return Err(RuntimeError::InvalidSigCount)
                }

                let mut signatures: Vec<Signature> = Vec::with_capacity(m);
                for _ in 0..m {
                    signatures.push(Signature::from(stack.pop()?.assert_len(SIGNATURE_LENGTH)?.as_ref()));
                }

                let tag = Tag::try_new(stack.pop()?.as_ref().to_vec())?;
                let tx_hash = tx.hash_partially(tag, cur, burn)?;

                let result = if check_multi_sig(&tx_hash, pubkey, signatures) {
                    1
                } else {
                    0
                };
                stack.push(Item(vec![result]))?;
            }
            Instruction::Blake256 => {
                let value = stack.pop()?;
                stack.push(Item(blake256(value).to_vec()))?;
            }
            Instruction::Sha256 => {
                let value = stack.pop()?;
                stack.push(Item(sha256(value).to_vec()))?;
            }
            Instruction::Ripemd160 => {
                let value = stack.pop()?;
                stack.push(Item(ripemd160(value).to_vec()))?;
            }
            Instruction::Keccak256 => {
                let value = stack.pop()?;
                stack.push(Item(keccak256(value).to_vec()))?;
            }
            Instruction::Blake160 => {
                let value = stack.pop()?;
                stack.push(Item(H160::blake(value).to_vec()))?;
            }
            Instruction::ChkTimelock => {
                let timelock_type = stack.pop()?.assert_len(1)?.as_ref()[0] as u8;
                let value_item = stack.pop()?;
                if value_item.len() > 8 {
                    return Err(RuntimeError::TypeMismatch)
                }
                let value = BigEndian::read_uint(value_item.as_ref(), value_item.len());
                match timelock_type {
                    TIMELOCK_TYPE_BLOCK => {
                        stack.push(Item::from(client.best_block_number() >= value))?;
                    }
                    TIMELOCK_TYPE_BLOCK_AGE => {
                        stack.push(Item::from(
                            client
                                .transaction_block_age(&cur.prev_out.transaction_hash)
                                .map_or(false, |age| age >= value),
                        ))?;
                    }
                    TIMELOCK_TYPE_TIME => {
                        stack.push(Item::from(client.best_block_timestamp() >= value))?;
                    }
                    TIMELOCK_TYPE_TIME_AGE => {
                        stack.push(Item::from(
                            client
                                .transaction_time_age(&cur.prev_out.transaction_hash)
                                .map_or(false, |age| age >= value),
                        ))?;
                    }
                    _ => return Err(RuntimeError::InvalidTimelockType),
                }
            }
        }
        pc += 1;
    }

    let result = stack.pop()?;
    if result.into() && stack.len() == 0 {
        Ok(ScriptResult::Unlocked)
    } else {
        Ok(ScriptResult::Fail)
    }
}

#[inline]
fn check_multi_sig(tx_hash: &H256, mut pubkey: Vec<Public>, mut signatures: Vec<Signature>) -> bool {
    while let Some(sig) = signatures.pop() {
        loop {
            let public = match pubkey.pop() {
                None => return false,
                Some(public) => public,
            };
            if verify(&public, &sig, &tx_hash) == Ok(true) {
                break
            }
        }
    }
    true
}

pub trait ChainTimeInfo {
    /// Get the best block number.
    fn best_block_number(&self) -> u64;

    /// Get the best block timestamp.
    fn best_block_timestamp(&self) -> u64;

    /// Get the block height of the transaction.
    fn transaction_block_age(&self, hash: &H256) -> Option<u64>;

    /// Get the how many seconds elapsed since transaction is confirmed, according to block timestamp.
    fn transaction_time_age(&self, hash: &H256) -> Option<u64>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_true() {
        let item: Item = true.into();
        assert_eq!(vec![1], item.as_ref());
        let result: bool = item.into();
        assert!(result);
    }

    #[test]
    fn convert_false() {
        let item: Item = false.into();
        assert_eq!(Vec::<u8>::new(), item.as_ref());
        let result: bool = item.into();
        assert!(!result);
    }

    #[test]
    fn false_if_all_bit_is_zero() {
        let item = Item(vec![0, 0, 0, 0, 0, 0, 0]);
        let result: bool = item.into();
        assert!(!result);
    }

    #[test]
    fn true_if_at_least_one_bit_is_not_zero() {
        let item = Item(vec![0, 0, 0, 1, 0, 0, 0]);
        let result: bool = item.into();
        assert!(result);
    }
}

#[cfg(test)]
mod tests_check_multi_sig {
    use ckey::{sign, Generator, Random};

    use super::*;

    #[test]
    fn valid_2_of_3_110() {
        let key_pair1 = Random.generate().unwrap();
        let key_pair2 = Random.generate().unwrap();
        let key_pair3 = Random.generate().unwrap();
        let pubkey1 = *key_pair1.public();
        let pubkey2 = *key_pair2.public();
        let pubkey3 = *key_pair3.public();
        let message = H256::random();
        let signature1 = sign(key_pair1.private(), &message).unwrap();
        let signature2 = sign(key_pair2.private(), &message).unwrap();

        assert!(check_multi_sig(&message, vec![pubkey1, pubkey2, pubkey3], vec![signature1, signature2]));
    }

    #[test]
    fn valid_2_of_3_101() {
        let key_pair1 = Random.generate().unwrap();
        let key_pair2 = Random.generate().unwrap();
        let key_pair3 = Random.generate().unwrap();
        let pubkey1 = *key_pair1.public();
        let pubkey2 = *key_pair2.public();
        let pubkey3 = *key_pair3.public();
        let message = H256::random();
        let signature1 = sign(key_pair1.private(), &message).unwrap();
        let signature3 = sign(key_pair3.private(), &message).unwrap();

        assert!(check_multi_sig(&message, vec![pubkey1, pubkey2, pubkey3], vec![signature1, signature3]));
    }

    #[test]
    fn valid_2_of_3_011() {
        let key_pair1 = Random.generate().unwrap();
        let key_pair2 = Random.generate().unwrap();
        let key_pair3 = Random.generate().unwrap();
        let pubkey1 = *key_pair1.public();
        let pubkey2 = *key_pair2.public();
        let pubkey3 = *key_pair3.public();
        let message = H256::random();
        let signature2 = sign(key_pair2.private(), &message).unwrap();
        let signature3 = sign(key_pair3.private(), &message).unwrap();

        assert!(check_multi_sig(&message, vec![pubkey1, pubkey2, pubkey3], vec![signature2, signature3]));
    }

    #[test]
    fn invalid_2_of_2_if_order_is_different() {
        let key_pair1 = Random.generate().unwrap();
        let key_pair2 = Random.generate().unwrap();
        let pubkey1 = *key_pair1.public();
        let pubkey2 = *key_pair2.public();
        let message = H256::random();
        let signature1 = sign(key_pair1.private(), &message).unwrap();
        let signature2 = sign(key_pair2.private(), &message).unwrap();

        assert!(!check_multi_sig(&message, vec![pubkey2, pubkey1], vec![signature1, signature2]));
    }
}

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

use ccrypto::{blake160, blake256, keccak256, ripemd160, sha256, Blake};
use ckey::{recover, verify, Public, Signature, SIGNATURE_LENGTH};
use ctypes::transaction::{AssetOutPoint, HashingError, PartialHashing};
use ctypes::util::tag::Tag;

use primitives::H160;


use instruction::{has_expensive_opcodes, is_valid_unlock_script, Instruction};

const DEFAULT_MAX_MEMORY: usize = 1024;

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

pub fn execute(
    unlock: &[Instruction],
    params: &[Vec<u8>],
    lock: &[Instruction],
    tx: &PartialHashing,
    config: Config,
    cur: &AssetOutPoint,
    burn: bool,
) -> Result<ScriptResult, RuntimeError> {
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
                let signature = Signature::from(Signature::from(stack.pop()?.assert_len(SIGNATURE_LENGTH)?.as_ref()));
                let result = match verify(&pubkey, &signature, &tx_hash) {
                    Ok(true) => 1,
                    _ => 0,
                };
                stack.push(Item(vec![result]))?;
            }
            Instruction::ChkMultiSig => {
                // Get n pubkey. If there are more than six pubkeys, return error.
                let n = stack.pop()?.assert_len(1)?.as_ref()[0] as usize;
                let mut pubkey: Vec<Public> = Vec::new();
                for _ in 0..n {
                    pubkey.push(Public::from_slice(stack.pop()?.assert_len(64)?.as_ref()));
                }

                // Get m signature. If signatures are more than pubkeys, return error.
                let m = stack.pop()?.assert_len(1)?.as_ref()[0] as usize;
                if m > n || m == 0 || m > 6 {
                    return Err(RuntimeError::InvalidSigCount)
                }
                let mut signatures: Vec<Signature> = Vec::new();
                for _ in 0..m {
                    signatures.push(Signature::from(stack.pop()?.assert_len(SIGNATURE_LENGTH)?.as_ref()));
                }

                let tag = Tag::try_new(stack.pop()?.as_ref().to_vec())?;
                let tx_hash = tx.hash_partially(tag, cur, burn)?;

                let mut result = 1;
                while let Some(sig) = signatures.pop() {
                    let public = pubkey.pop().unwrap();
                    if let Ok(false) = verify(&public, &sig, &tx_hash) {
                        result = 0;
                        break
                    }
                }
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
                stack.push(Item(blake160(value).to_vec()))?;
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

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

use ckeys::{verify_ecdsa, ECDSASignature};
use ctypes::{H256, H520, Public};

use instruction::Instruction;

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
    StackUnderflow,
    TypeMismatch,
    InvalidResult,
}

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
}

pub fn execute(script: &[Instruction], tx_hash: H256, config: Config) -> Result<ScriptResult, RuntimeError> {
    let mut stack = Stack::new(config);
    let mut pc = 0;
    while pc < script.len() {
        match &script[pc] {
            Instruction::Nop => {}
            Instruction::PushB(blob) => stack.push(Item(blob.clone()))?,
            Instruction::PushI(val) => stack.push(Item(vec![*val as u8]))?,
            Instruction::Pop => {
                stack.pop()?;
            }
            Instruction::ChkSig => {
                let pubkey = Public::from_slice(stack.pop()?.assert_len(64)?.as_ref());
                let signature = ECDSASignature::from(H520::from(stack.pop()?.assert_len(65)?.as_ref()));
                let result = match verify_ecdsa(&pubkey, &signature, &tx_hash) {
                    Ok(true) => 1,
                    _ => 0,
                };
                stack.push(Item(vec![result]))?;
            }
        }
        pc += 1;
    }

    let result = stack.pop()?;
    // FIXME: convert stack top value to integer value
    if result.as_ref() != [0] && stack.len() == 0 {
        Ok(ScriptResult::Unlocked)
    } else {
        Ok(ScriptResult::Fail)
    }
}

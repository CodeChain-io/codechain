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

enum Item {
    Integer(i8),
    Blob(Vec<u8>),
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
        let item_size = match &val {
            &Item::Integer(..) => 1,
            &Item::Blob(ref blob) => blob.len(),
        };

        if self.memory_usage + item_size > self.config.max_memory {
            Err(RuntimeError::OutOfMemory)
        } else {
            self.memory_usage += item_size;
            self.stack.push(val);
            Ok(())
        }
    }

    fn pop(&mut self) -> Result<Item, RuntimeError> {
        let item = self.stack.pop();
        let item_size = match &item {
            &Some(Item::Integer(..)) => 1,
            &Some(Item::Blob(ref blob)) => blob.len(),
            &None => 0,
        };
        self.memory_usage -= item_size;
        item.ok_or(RuntimeError::StackUnderflow)
    }

    fn pop_blob(&mut self, len: usize) -> Result<Vec<u8>, RuntimeError> {
        if let Item::Blob(blob) = self.pop()? {
            if blob.len() == len {
                return Ok(blob)
            }
        }
        Err(RuntimeError::TypeMismatch)
    }

    fn len(&self) -> usize {
        self.stack.len()
    }
}

pub fn execute(script: &[Instruction], tx_hash: H256, config: Config) -> Result<ScriptResult, RuntimeError> {
    let mut stack = Stack::new(config);
    let mut pc = 0;
    while pc < script.len() {
        match script[pc] {
            Instruction::Nop => {}
            Instruction::PushB(ref blob) => stack.push(Item::Blob(blob.clone()))?,
            Instruction::PushI(val) => stack.push(Item::Integer(val))?,
            Instruction::Pop => {
                stack.pop()?;
            }
            Instruction::ChkSig => {
                let pubkey = Public::from_slice(stack.pop_blob(64)?.as_slice());
                let signature = ECDSASignature::from(H520::from(stack.pop_blob(65)?.as_slice()));
                let result = match verify_ecdsa(&pubkey, &signature, &tx_hash) {
                    Ok(true) => 1,
                    _ => 0,
                };
                stack.push(Item::Integer(result))?;
            }
        }
        pc += 1;
    }

    match stack.pop() {
        Ok(Item::Integer(result)) if stack.len() == 0 => {
            if result == 0 {
                Ok(ScriptResult::Fail)
            } else {
                Ok(ScriptResult::Unlocked)
            }
        }
        _ => Err(RuntimeError::InvalidResult),
    }
}

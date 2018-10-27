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

use instruction::Instruction;
use opcode;

#[derive(Debug, PartialEq)]
pub enum DecoderError {
    ScriptTooShort,
    InvalidOpCode(u8),
}

pub fn decode(bytes: &[u8]) -> Result<Vec<Instruction>, DecoderError> {
    let mut iter = bytes.iter();
    let mut result = Vec::new();
    while let Some(b) = iter.next() {
        match *b {
            opcode::NOP => result.push(Instruction::Nop),
            opcode::BURN => result.push(Instruction::Burn),
            opcode::SUCCESS => result.push(Instruction::Success),
            opcode::FAIL => result.push(Instruction::Fail),
            opcode::NOT => result.push(Instruction::Not),
            opcode::EQ => result.push(Instruction::Eq),
            opcode::JMP => {
                let val = *iter.next().ok_or(DecoderError::ScriptTooShort)?;
                result.push(Instruction::Jmp(val));
            }
            opcode::JNZ => {
                let val = *iter.next().ok_or(DecoderError::ScriptTooShort)?;
                result.push(Instruction::Jnz(val));
            }
            opcode::JZ => {
                let val = *iter.next().ok_or(DecoderError::ScriptTooShort)?;
                result.push(Instruction::Jz(val));
            }
            opcode::PUSH => {
                let val = *iter.next().ok_or(DecoderError::ScriptTooShort)?;
                result.push(Instruction::Push(val));
            }
            opcode::POP => result.push(Instruction::Pop),
            opcode::PUSHB => {
                let len = *iter.next().ok_or(DecoderError::ScriptTooShort)?;
                // FIXME : optimize blob assignment
                let mut blob = Vec::new();
                for _ in 0..len {
                    blob.push(*iter.next().ok_or(DecoderError::ScriptTooShort)?);
                }
                result.push(Instruction::PushB(blob));
            }
            opcode::DUP => result.push(Instruction::Dup),
            opcode::SWAP => result.push(Instruction::Swap),
            opcode::COPY => {
                let val = *iter.next().ok_or(DecoderError::ScriptTooShort)?;
                result.push(Instruction::Copy(val));
            }
            opcode::DROP => {
                let val = *iter.next().ok_or(DecoderError::ScriptTooShort)?;
                result.push(Instruction::Drop(val));
            }
            opcode::CHKSIG => result.push(Instruction::ChkSig),
            opcode::CHKMULTISIG => result.push(Instruction::ChkMultiSig),
            opcode::BLAKE256 => result.push(Instruction::Blake256),
            opcode::SHA256 => result.push(Instruction::Sha256),
            opcode::RIPEMD160 => result.push(Instruction::Ripemd160),
            opcode::KECCAK256 => result.push(Instruction::Keccak256),
            opcode::BLAKE160 => result.push(Instruction::Blake160),
            opcode::CHKTIMELOCK => result.push(Instruction::ChkTimelock),
            invalid_opcode => return Err(DecoderError::InvalidOpCode(invalid_opcode)),
        }
    }

    Ok(result)
}

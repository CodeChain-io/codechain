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

use crate::instruction::Instruction;
use crate::opcode;

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

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_no_argument_opcode {
        ($opcode:ident, $instruction:ident) => {
            #[test]
            #[allow(non_snake_case)]
            fn $instruction() {
                assert_eq!(decode(&[opcode::$opcode]), Ok(vec![Instruction::$instruction]));
                assert_eq!(
                    decode(&[opcode::$opcode, opcode::$opcode]),
                    Ok(vec![Instruction::$instruction, Instruction::$instruction])
                );
            }
        };
    }

    macro_rules! test_one_argument_opcode {
        ($opcode:ident, $instruction:ident) => {
            #[test]
            #[allow(non_snake_case)]
            fn $instruction() {
                assert_eq!(decode(&[opcode::$opcode]), Err(DecoderError::ScriptTooShort));
                assert_eq!(decode(&[opcode::$opcode, 0]), Ok(vec![Instruction::$instruction(0)]));
                assert_eq!(decode(&[opcode::$opcode, 0, opcode::$opcode]), Err(DecoderError::ScriptTooShort));
                assert_eq!(
                    decode(&[opcode::$opcode, 0, opcode::$opcode, 1]),
                    Ok(vec![Instruction::$instruction(0), Instruction::$instruction(1)])
                );
            }
        };
    }

    test_no_argument_opcode!(NOP, Nop);
    test_no_argument_opcode!(BURN, Burn);
    test_no_argument_opcode!(SUCCESS, Success);
    test_no_argument_opcode!(FAIL, Fail);
    test_no_argument_opcode!(NOT, Not);
    test_no_argument_opcode!(EQ, Eq);
    test_one_argument_opcode!(JMP, Jmp);
    test_one_argument_opcode!(JNZ, Jnz);
    test_one_argument_opcode!(JZ, Jz);
    test_one_argument_opcode!(PUSH, Push);
    test_no_argument_opcode!(POP, Pop);
    test_no_argument_opcode!(DUP, Dup);
    test_no_argument_opcode!(SWAP, Swap);
    test_one_argument_opcode!(COPY, Copy);
    test_one_argument_opcode!(DROP, Drop);
    test_no_argument_opcode!(CHKSIG, ChkSig);
    test_no_argument_opcode!(CHKMULTISIG, ChkMultiSig);
    test_no_argument_opcode!(BLAKE256, Blake256);
    test_no_argument_opcode!(SHA256, Sha256);
    test_no_argument_opcode!(RIPEMD160, Ripemd160);
    test_no_argument_opcode!(KECCAK256, Keccak256);
    test_no_argument_opcode!(BLAKE160, Blake160);
    test_no_argument_opcode!(CHKTIMELOCK, ChkTimelock);

    #[test]
    #[allow(non_snake_case)]
    fn PushB() {
        let blobs: Vec<&[u8]> = vec![&[0xed, 0x11, 0xe7], &[0x8b, 0x0c, 0x92, 0x24, 0x3f]];
        assert_eq!(
            decode([&[opcode::PUSHB, 3], &blobs[0][..], &[opcode::PUSHB, 5], &blobs[1][..]].concat().as_slice()),
            Ok(vec![Instruction::PushB(blobs[0].to_vec()), Instruction::PushB(blobs[1].to_vec())])
        );
        assert_eq!(decode([&[opcode::PUSHB, 4], &blobs[0][..]].concat().as_slice()), Err(DecoderError::ScriptTooShort));
    }
}

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

use decoder::{decode, DecoderError};
use instruction::Instruction;
use opcode;

#[test]
fn test_single_byte_opcodes() {
    let target = [
        (opcode::NOP, Instruction::Nop),
        (opcode::NOT, Instruction::Not),
        (opcode::EQ, Instruction::Eq),
        (opcode::POP, Instruction::Pop),
        (opcode::DUP, Instruction::Dup),
        (opcode::SWAP, Instruction::Swap),
        (opcode::CHKSIG, Instruction::ChkSig),
        (opcode::BLAKE256, Instruction::Blake256),
        (opcode::SHA256, Instruction::Sha256),
        (opcode::RIPEMD160, Instruction::Ripemd160),
    ];
    for &(ref byte, ref code) in target.into_iter() {
        let script = decode(&[byte.clone(), byte.clone(), byte.clone()]);
        assert_eq!(script, Ok(vec![code.clone(), code.clone(), code.clone()]));
    }
}

#[test]
fn push() {
    assert_eq!(decode(&[opcode::PUSH, 0, opcode::PUSH, 10]), Ok(vec![Instruction::Push(0), Instruction::Push(10)]));
    assert_eq!(decode(&[opcode::PUSH, 0, opcode::PUSH]), Err(DecoderError::ScriptTooShort));
}

#[test]
fn copy() {
    assert_eq!(decode(&[opcode::COPY, 0, opcode::COPY, 10]), Ok(vec![Instruction::Copy(0), Instruction::Copy(10)]));
    assert_eq!(decode(&[opcode::COPY, 0, opcode::COPY]), Err(DecoderError::ScriptTooShort));
}

#[test]
fn drop() {
    assert_eq!(decode(&[opcode::DROP, 0, opcode::DROP, 10]), Ok(vec![Instruction::Drop(0), Instruction::Drop(10)]));
    assert_eq!(decode(&[opcode::DROP, 0, opcode::DROP]), Err(DecoderError::ScriptTooShort));
}

#[test]
fn jmp() {
    assert_eq!(decode(&[opcode::JMP, 0, opcode::JMP, 10]), Ok(vec![Instruction::Jmp(0), Instruction::Jmp(10)]));
    assert_eq!(decode(&[opcode::JMP, 0, opcode::JMP]), Err(DecoderError::ScriptTooShort));
}

#[test]
fn jnz() {
    assert_eq!(decode(&[opcode::JNZ, 0, opcode::JNZ, 10]), Ok(vec![Instruction::Jnz(0), Instruction::Jnz(10)]));
    assert_eq!(decode(&[opcode::JNZ, 0, opcode::JNZ]), Err(DecoderError::ScriptTooShort));
}

#[test]
fn jz() {
    assert_eq!(decode(&[opcode::JZ, 0, opcode::JZ, 10]), Ok(vec![Instruction::Jz(0), Instruction::Jz(10)]));
    assert_eq!(decode(&[opcode::JZ, 0, opcode::JZ]), Err(DecoderError::ScriptTooShort));
}

#[test]
fn pushb() {
    let blobs: Vec<&[u8]> = vec![&[0xed, 0x11, 0xe7], &[0x8b, 0x0c, 0x92, 0x24, 0x3f]];
    assert_eq!(
        decode([&[opcode::PUSHB, 3], &blobs[0][..], &[opcode::PUSHB, 5], &blobs[1][..]].concat().as_slice()),
        Ok(vec![Instruction::PushB(blobs[0].to_vec()), Instruction::PushB(blobs[1].to_vec())])
    );
    assert_eq!(decode([&[opcode::PUSHB, 4], &blobs[0][..]].concat().as_slice()), Err(DecoderError::ScriptTooShort));
}

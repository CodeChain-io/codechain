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
test_no_argument_opcode!(BLAKE256, Blake256);
test_no_argument_opcode!(SHA256, Sha256);
test_no_argument_opcode!(RIPEMD160, Ripemd160);
test_no_argument_opcode!(KECCAK256, Keccak256);

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

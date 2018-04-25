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
use opcode;
use opcode::OpCode;

#[test]
fn test_single_byte_opcodes() {
    let target = [(opcode::NOP, OpCode::Nop), (opcode::POP, OpCode::Pop), (opcode::CHKSIG, OpCode::ChkSig)];
    for &(ref byte, ref code) in target.into_iter() {
        let script = decode(vec![byte.clone(), byte.clone(), byte.clone()]);
        assert_eq!(script, Ok(vec![code.clone(), code.clone(), code.clone()]));
    }
}

#[test]
fn pushi() {
    assert_eq!(decode(vec![opcode::PUSHI, 0, opcode::PUSHI, 10]), Ok(vec![OpCode::PushI(0), OpCode::PushI(10)]));
    assert_eq!(decode(vec![opcode::PUSHI, 0, opcode::PUSHI]), Err(DecoderError::ScriptTooShort));
}

#[test]
fn pushs() {
    let blobs = [vec![0xed, 0x11, 0xe7], vec![0x8b, 0x0c, 0x92, 0x24, 0x3f]];
    assert_eq!(
        decode([&[opcode::PUSHS, 3], &blobs[0][..], &[opcode::PUSHS, 5], &blobs[1][..]].concat()),
        Ok(vec![OpCode::PushS(blobs[0].clone()), OpCode::PushS(blobs[1].clone())])
    );
    assert_eq!(decode([&[opcode::PUSHS, 4], &blobs[0][..]].concat()), Err(DecoderError::ScriptTooShort));
}

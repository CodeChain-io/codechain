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

use cbytes::Bytes;

use opcode;
use opcode::OpCode;

pub enum DecoderError {
    ScriptTooShort,
    InvalidOpCode(u8),
}

pub fn decode(bytes: Bytes) -> Result<Vec<OpCode>, DecoderError> {
    let mut iter = bytes.into_iter();
    let mut result = Vec::new();
    while let Some(b) = iter.next() {
        match b {
            opcode::NOP => result.push(OpCode::Nop),
            opcode::PUSHS => {
                let len = iter.next().ok_or(DecoderError::ScriptTooShort)?;
                // FIXME : optimize blob assignment
                let mut blob = Vec::new();
                for _ in 0..len {
                    blob.push(iter.next().ok_or(DecoderError::ScriptTooShort)?);
                }
                result.push(OpCode::PushS(blob));
            }
            opcode::PUSHI => {
                let val = iter.next().ok_or(DecoderError::ScriptTooShort)?;
                result.push(OpCode::PushI(val as i8));
            }
            opcode::POP => result.push(OpCode::Pop),
            opcode::CHKSIG => result.push(OpCode::ChkSig),
            _ => return Err(DecoderError::InvalidOpCode(b)),
        }
    }

    Ok(result)
}

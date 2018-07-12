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

#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    Nop,
    Burn,
    Success,
    Fail,
    Not,
    Eq,
    Jmp(u8),
    Jnz(u8),
    Jz(u8),
    Push(u8),
    Pop,
    PushB(Vec<u8>),
    Dup,
    Swap,
    Copy(u8),
    Drop(u8),
    ChkSig,
    Blake256,
    Sha256,
    Ripemd160,
    Keccak256,
}

pub fn is_valid_unlock_script(instrs: &[Instruction]) -> bool {
    instrs.iter().all(|instr| match instr {
        Instruction::Push(_) => true,
        Instruction::PushB(_) => true,
        _ => false,
    })
}

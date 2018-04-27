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

pub const NOP: u8 = 0x00;
pub const PUSHB: u8 = 0x01;
pub const PUSHI: u8 = 0x02;
pub const POP: u8 = 0x03;
pub const CHKSIG: u8 = 0x04;

#[derive(Clone, Debug, PartialEq)]
pub enum OpCode {
    Nop,
    PushB(Vec<u8>),
    PushI(i8),
    Pop,
    ChkSig,
}

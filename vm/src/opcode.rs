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
pub const BURN: u8 = 0x01;
pub const SUCCESS: u8 = 0x02;
pub const FAIL: u8 = 0x03;
pub const NOT: u8 = 0x10;
pub const EQ: u8 = 0x11;
pub const JMP: u8 = 0x20;
pub const JNZ: u8 = 0x21;
pub const JZ: u8 = 0x22;
pub const PUSH: u8 = 0x30;
pub const POP: u8 = 0x31;
pub const PUSHB: u8 = 0x32;
pub const DUP: u8 = 0x33;
pub const SWAP: u8 = 0x34;
pub const COPY: u8 = 0x35;
pub const DROP: u8 = 0x36;
pub const CHKSIG: u8 = 0x80;
pub const CHKMULTISIG: u8 = 0x81;
pub const BLAKE256: u8 = 0x90;
pub const SHA256: u8 = 0x91;
pub const RIPEMD160: u8 = 0x92;
pub const KECCAK256: u8 = 0x93;
pub const BLAKE160: u8 = 0x94;
pub const CHKTIMELOCK: u8 = 0xb0;

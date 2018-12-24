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


use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "value")]
pub enum Timelock {
    Block(u64),
    BlockAge(u64),
    Time(u64),
    TimeAge(u64),
}

type TimelockType = u8;
const BLOCK: TimelockType = 0x01;
const BLOCK_AGE: TimelockType = 0x02;
const TIME: TimelockType = 0x03;
const TIME_AGE: TimelockType = 0x04;

impl Encodable for Timelock {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Timelock::Block(val) => s.begin_list(2).append(&BLOCK).append(val),
            Timelock::BlockAge(val) => s.begin_list(2).append(&BLOCK_AGE).append(val),
            Timelock::Time(val) => s.begin_list(2).append(&TIME).append(val),
            Timelock::TimeAge(val) => s.begin_list(2).append(&TIME_AGE).append(val),
        };
    }
}

impl Decodable for Timelock {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        if d.item_count()? != 2 {
            return Err(DecoderError::RlpIncorrectListLen)
        }
        match d.val_at(0)? {
            BLOCK => Ok(Timelock::Block(d.val_at(1)?)),
            BLOCK_AGE => Ok(Timelock::BlockAge(d.val_at(1)?)),
            TIME => Ok(Timelock::Time(d.val_at(1)?)),
            TIME_AGE => Ok(Timelock::TimeAge(d.val_at(1)?)),
            _ => Err(DecoderError::Custom("Unexpected timelock type")),
        }
    }
}

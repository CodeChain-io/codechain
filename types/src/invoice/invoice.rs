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

/// Information describing execution of a parcel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Invoice {
    Success,
    Failed,
}

impl Encodable for Invoice {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Invoice::Success => s.append_single_value(&1u8),
            Invoice::Failed => s.append_single_value(&0u8),
        };
    }
}

impl Decodable for Invoice {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        Ok(match rlp.as_val::<u8>()? {
            1 => Invoice::Success,
            0 => Invoice::Failed,
            _ => return Err(DecoderError::Custom("Invalid parcel outcome")),
        })
    }
}

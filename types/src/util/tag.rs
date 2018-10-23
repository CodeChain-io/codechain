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

use super::super::transaction::HashingError;

pub struct Tag {
    pub sign_all_inputs: bool,
    pub sign_all_outputs: bool,
    pub filter_len: usize,
    pub filter: Vec<u8>,
    pub bitvec: Vec<u8>,
}

impl Tag {
    pub fn try_new(mut bitvec: Vec<u8>) -> Result<Tag, HashingError> {
        let vec = bitvec.clone();
        let tag = bitvec.pop().ok_or(HashingError::InvalidFilter)?;
        let sign_all_inputs = (tag & 0x1) == 1;
        let sign_all_outputs = (tag >> 1 & 0x1) == 1;
        let filter_len = (tag >> 2) as usize;

        if bitvec.len() != filter_len {
            return Err(HashingError::InvalidFilter)
        }

        Ok(Tag {
            sign_all_inputs,
            sign_all_outputs,
            filter_len,
            filter: bitvec,
            bitvec: vec,
        })
    }

    pub fn get_tag(&self) -> &Vec<u8> {
        &self.bitvec
    }
}
#[cfg(test)]
mod tests {
    use util::tag::Tag;
    #[test]
    fn make_partial_signing_tag() {
        let bitvec = vec![
            0b10000000, 0b01000000, 0b00100000, 0b00010000, 0b00001000, 0b00000100, 0b00000010, 0b00000001, 0b00100001,
        ];
        let tag = Tag::try_new(bitvec).unwrap();

        assert_eq!(tag.sign_all_inputs, true);
        assert_eq!(tag.sign_all_outputs, false);
        assert_eq!(tag.filter_len, 8);
        assert_eq!(
            tag.filter.clone(),
            vec![0b10000000, 0b01000000, 0b00100000, 0b00010000, 0b00001000, 0b00000100, 0b00000010, 0b00000001]
        );
    }
}

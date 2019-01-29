// Copyright 2019. Kodebox, Inc.
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

use rlp::{DecoderError, Encodable, RlpStream, UntrustedRlp};

mod history_error;
mod runtime_error;
mod syntax_error;

pub use self::history_error::Error as HistoryError;
pub use self::runtime_error::{Error as RuntimeError, UnlockFailureReason};
pub use self::syntax_error::Error as SyntaxError;

trait TaggedRlp {
    type Tag: Encodable + Copy;

    fn length_of(tag: Self::Tag) -> Result<usize, DecoderError>;

    fn new_tagged_list(s: &mut RlpStream, tag: Self::Tag) -> &mut RlpStream {
        s.begin_list(Self::length_of(tag).unwrap()).append(&tag)
    }

    fn check_size(rlp: &UntrustedRlp, tag: Self::Tag) -> Result<(), DecoderError> {
        if rlp.item_count()? != Self::length_of(tag)? {
            return Err(DecoderError::RlpInvalidLength)
        }
        Ok(())
    }
}

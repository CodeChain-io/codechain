// Copyright 2018-2019 Kodebox, Inc.
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

use ccore::UnverifiedTransaction;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Debug, PartialEq)]
pub enum Message {
    Transactions(Vec<UnverifiedTransaction>),
}

impl Encodable for Message {
    fn rlp_append(&self, s: &mut RlpStream) {
        match &self {
            Message::Transactions(transactions) => s.append_list(transactions),
        };
    }
}

impl Decodable for Message {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        Ok(Message::Transactions(rlp.as_list()?))
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::Message;

    #[test]
    fn transactions_message_rlp() {
        rlp_encode_and_decode_test!(Message::Transactions(Vec::new()));
    }
}

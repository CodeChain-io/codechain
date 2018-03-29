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

use rlp::{Encodable, Decodable, DecoderError, RlpStream, UntrustedRlp};

/// Information describing execution of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invoice {
    /// Transaction outcome.
    pub outcome: TransactionOutcome,
}

/// Transaction outcome store in the invoice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionOutcome {
    Success,
    Failed,
}

impl Invoice {
    /// Create a new invocie.
    pub fn new(outcome: TransactionOutcome) -> Self {
        Self {
            outcome
        }
    }
}

impl Encodable for Invoice {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self.outcome {
            TransactionOutcome::Success => s.append(&1u8),
            TransactionOutcome::Failed => s.append(&0u8),
        };
    }
}

impl Decodable for Invoice {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let outcome = match rlp.as_val::<u8>()? {
            1 => TransactionOutcome::Success,
            0 => TransactionOutcome::Failed,
            _ => return Err(DecoderError::Custom("Invalid transaction outcome")),
        };
        Ok(Self { outcome })
    }
}


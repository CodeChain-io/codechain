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
// A state machine.

use super::block::ExecutedBlock;
use super::client::BlockInfo;
use super::error::Error;
use super::header::Header;
use super::transaction::{UnverifiedTransaction, SignedTransaction};

pub struct CodeChainMachine {
}

impl CodeChainMachine {
    pub fn new() -> Self {
        CodeChainMachine {
        }
    }

    /// Does basic verification of the transaction.
    pub fn verify_transaction_basic(&self, t: &UnverifiedTransaction, header: &Header) -> Result<(), Error> {
        t.verify_basic(false)?;

        Ok(())
    }

    /// Verify a particular transaction is valid, regardless of order.
    pub fn verify_transaction_unordered(&self, t: UnverifiedTransaction, _header: &Header) -> Result<SignedTransaction, Error> {
        Ok(SignedTransaction::new(t)?)
    }

    /// Does verification of the transaction against the parent state.
    pub fn verify_transaction<C: BlockInfo>(&self, t: &SignedTransaction, header: &Header, client: &C) -> Result<(), Error> {
        // FIXME: Filter transactions.
        Ok(())
    }
}

impl ::machine::Machine for CodeChainMachine {
    type Header = Header;
    type LiveBlock = ExecutedBlock;
    type EngineClient = super::client::EngineClient;

    type Error = Error;
}


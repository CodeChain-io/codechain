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

use ckeys::Address;
use ctypes::U256;

use super::block::{ExecutedBlock, IsBlock};
use super::client::BlockInfo;
use super::error::Error;
use super::header::Header;
use super::spec::CommonParams;
use super::transaction::{SignedTransaction, TransactionError, UnverifiedTransaction};

pub struct CodeChainMachine {
    params: CommonParams,
}

impl CodeChainMachine {
    pub fn new(params: CommonParams) -> Self {
        CodeChainMachine {
            params,
        }
    }

    /// Get the general parameters of the chain.
    pub fn params(&self) -> &CommonParams {
        &self.params
    }

    /// Does basic verification of the transaction.
    pub fn verify_transaction_basic(&self, t: &UnverifiedTransaction, _header: &Header) -> Result<(), Error> {
        if t.fee < self.params.min_transaction_cost {
            return Err(TransactionError::InsufficientFee {
                minimal: self.params.min_transaction_cost,
                got: t.fee,
            }.into())
        }
        t.verify_basic(self.params().network_id, false)?;

        Ok(())
    }

    /// Verify a particular transaction is valid, regardless of order.
    pub fn verify_transaction_unordered(
        &self,
        t: UnverifiedTransaction,
        _header: &Header,
    ) -> Result<SignedTransaction, Error> {
        Ok(SignedTransaction::new(t)?)
    }

    /// Does verification of the transaction against the parent state.
    pub fn verify_transaction<C: BlockInfo>(
        &self,
        _t: &SignedTransaction,
        header: &Header,
        _client: &C,
    ) -> Result<(), Error> {
        // FIXME: Filter transactions.
        Ok(())
    }

    /// The nonce with which accounts begin at given block.
    pub fn account_start_nonce(&self) -> U256 {
        self.params.account_start_nonce
    }
}

impl ::machine::Machine for CodeChainMachine {
    type Header = Header;
    type LiveBlock = ExecutedBlock;
    type EngineClient = super::client::EngineClient;

    type Error = Error;

    fn balance(&self, live: &ExecutedBlock, address: &Address) -> Result<U256, Self::Error> {
        live.state().balance(address).map_err(Into::into)
    }

    fn add_balance(&self, live: &mut ExecutedBlock, address: &Address, amount: &U256) -> Result<(), Self::Error> {
        live.state_mut().add_balance(address, amount).map_err(Into::into)
    }
}

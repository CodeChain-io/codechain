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

use ckey::Address;
use ctypes::U256;

use super::block::{ExecutedBlock, IsBlock};
use super::client::BlockInfo;
use super::error::Error;
use super::header::Header;
use super::parcel::{ParcelError, SignedParcel, UnverifiedParcel};
use super::spec::CommonParams;
use super::state::{TopState, TopStateInfo};

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

    /// Some intrinsic operation parameters; by default they take their value from the `spec()`'s `engine_params`.
    pub fn max_extra_data_size(&self) -> usize {
        self.params().max_extra_data_size
    }

    pub fn max_metadata_size(&self) -> usize {
        self.params().max_metadata_size
    }

    /// Does basic verification of the parcel.
    pub fn verify_parcel_basic(&self, t: &UnverifiedParcel, _header: &Header) -> Result<(), Error> {
        if t.fee < self.params.min_parcel_cost {
            return Err(ParcelError::InsufficientFee {
                minimal: self.params.min_parcel_cost,
                got: t.fee,
            }.into())
        }
        t.verify_basic(self.params(), false)?;

        Ok(())
    }

    /// Verify a particular parcel is valid, regardless of order.
    pub fn verify_parcel_unordered(&self, t: UnverifiedParcel, _header: &Header) -> Result<SignedParcel, Error> {
        Ok(SignedParcel::new(t)?)
    }

    /// Does verification of the parcel against the parent state.
    pub fn verify_parcel<C: BlockInfo>(&self, _t: &SignedParcel, _header: &Header, _client: &C) -> Result<(), Error> {
        // FIXME: Filter parcels.
        Ok(())
    }

    /// Populate a header's fields based on its parent's header.
    /// Usually implements the chain scoring rule based on weight.
    pub fn populate_from_parent(&self, header: &mut Header, parent: &Header) {
        header.set_score(parent.score().clone());
    }
}

impl ::machine::Machine for CodeChainMachine {
    type Header = Header;
    type LiveBlock = ExecutedBlock;
    type EngineClient = super::client::EngineClient;

    type Error = Error;
}

impl ::machine::WithBalances for CodeChainMachine {
    fn balance(&self, live: &ExecutedBlock, address: &Address) -> Result<U256, Self::Error> {
        live.state().balance(address).map_err(Into::into)
    }

    fn add_balance(&self, live: &mut ExecutedBlock, address: &Address, amount: &U256) -> Result<(), Self::Error> {
        live.state_mut().add_balance(address, amount).map_err(Into::into)
    }
}

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
use cstate::{StateError, TopState, TopStateView};
use ctypes::machine::{Machine, WithBalances};
use ctypes::parcel::{Action, Error as ParcelError};
use ctypes::transaction::{Error as TransactionError, Timelock, Transaction};

use crate::block::{ExecutedBlock, IsBlock};
use crate::client::{BlockInfo, TransactionInfo};
use crate::error::Error;
use crate::header::Header;
use crate::parcel::{SignedParcel, UnverifiedParcel};
use crate::scheme::CommonParams;

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

    pub fn max_text_content_size(&self) -> usize {
        self.params().max_text_content_size
    }

    /// Does basic verification of the parcel.
    pub fn verify_parcel_basic(&self, p: &UnverifiedParcel, _header: &Header) -> Result<(), Error> {
        if p.fee < self.params.min_parcel_cost {
            return Err(StateError::Parcel(ParcelError::InsufficientFee {
                minimal: self.params.min_parcel_cost,
                got: p.fee,
            })
            .into())
        }
        p.verify_basic(self.params()).map_err(StateError::from)?;

        Ok(())
    }

    /// Verify a particular parcel is valid, regardless of order.
    pub fn verify_parcel_unordered(&self, p: UnverifiedParcel, _header: &Header) -> Result<SignedParcel, Error> {
        Ok(SignedParcel::try_new(p)?)
    }

    /// Does verification of the parcel against the parent state.
    pub fn verify_parcel<C: BlockInfo + TransactionInfo>(
        &self,
        parcel: &SignedParcel,
        header: &Header,
        client: &C,
        verify_timelock: bool,
    ) -> Result<(), Error> {
        if let Action::AssetTransaction {
            transaction,
            ..
        } = &parcel.action
        {
            if verify_timelock {
                Self::verify_transaction_timelock(transaction, header, client)?;
            }
            Self::verify_transaction_order_expired(transaction, header)?;
        }
        // FIXME: Filter parcels.
        Ok(())
    }

    /// Populate a header's fields based on its parent's header.
    /// Usually implements the chain scoring rule based on weight.
    pub fn populate_from_parent(&self, header: &mut Header, parent: &Header) {
        header.set_score(*parent.score());
    }

    fn verify_transaction_timelock<C: BlockInfo + TransactionInfo>(
        transaction: &Transaction,
        header: &Header,
        client: &C,
    ) -> Result<(), Error> {
        let inputs = match transaction {
            Transaction::AssetTransfer {
                inputs,
                ..
            } => inputs,
            _ => return Ok(()),
        };
        for input in inputs {
            if let Some(timelock) = input.timelock {
                match timelock {
                    Timelock::Block(value) if value > header.number() => {
                        return Err(StateError::Transaction(TransactionError::Timelocked {
                            timelock,
                            remaining_time: value - header.number(),
                        })
                        .into())
                    }
                    Timelock::BlockAge(value) => {
                        let absolute =
                            client.transaction_block_number(&input.prev_out.transaction_hash).ok_or_else(|| {
                                Error::State(StateError::Transaction(TransactionError::Timelocked {
                                    timelock,
                                    remaining_time: u64::max_value(),
                                }))
                            })? + value;
                        if absolute > header.number() {
                            return Err(StateError::Transaction(TransactionError::Timelocked {
                                timelock,
                                remaining_time: absolute - header.number(),
                            })
                            .into())
                        }
                    }
                    Timelock::Time(value) if value > header.timestamp() => {
                        return Err(StateError::Transaction(TransactionError::Timelocked {
                            timelock,
                            remaining_time: value - header.timestamp(),
                        })
                        .into())
                    }
                    Timelock::TimeAge(value) => {
                        let absolute =
                            client.transaction_block_timestamp(&input.prev_out.transaction_hash).ok_or_else(|| {
                                Error::State(StateError::Transaction(TransactionError::Timelocked {
                                    timelock,
                                    remaining_time: u64::max_value(),
                                }))
                            })? + value;
                        if absolute > header.timestamp() {
                            return Err(StateError::Transaction(TransactionError::Timelocked {
                                timelock,
                                remaining_time: absolute - header.timestamp(),
                            })
                            .into())
                        }
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    }

    fn verify_transaction_order_expired(transaction: &Transaction, header: &Header) -> Result<(), Error> {
        let orders = match transaction {
            Transaction::AssetTransfer {
                orders,
                ..
            } => orders,
            _ => return Ok(()),
        };
        for order_tx in orders {
            if order_tx.order.expiration < header.timestamp() {
                return Err(StateError::Transaction(TransactionError::OrderExpired {
                    expiration: order_tx.order.expiration,
                    timestamp: header.timestamp(),
                })
                .into())
            }
        }
        Ok(())
    }
}

impl Machine for CodeChainMachine {
    type Header = Header;
    type LiveBlock = ExecutedBlock;
    type EngineClient = crate::client::EngineClient;

    type Error = Error;
}

impl WithBalances for CodeChainMachine {
    fn balance(&self, live: &ExecutedBlock, address: &Address) -> Result<u64, Self::Error> {
        Ok(live.state().balance(address).map_err(StateError::from)?)
    }

    fn add_balance(&self, live: &mut ExecutedBlock, address: &Address, amount: u64) -> Result<(), Self::Error> {
        live.state_mut().add_balance(address, amount).map_err(StateError::from)?;
        Ok(())
    }
}

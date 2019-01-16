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
// A state machine.

use ckey::Address;
use cstate::{StateError, TopState, TopStateView};
use ctypes::machine::{Machine, WithBalances};
use ctypes::transaction::{
    Action, AssetTransferInput, Error as TransactionError, OrderOnTransfer, ParcelError, Timelock,
};

use crate::block::{ExecutedBlock, IsBlock};
use crate::client::{BlockInfo, TransactionInfo};
use crate::error::Error;
use crate::header::Header;
use crate::scheme::CommonParams;
use crate::transaction::{SignedTransaction, UnverifiedTransaction};

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

    pub fn max_asset_scheme_metadata_size(&self) -> usize {
        self.params().max_asset_scheme_metadata_size
    }

    pub fn max_transfer_metadata_size(&self) -> usize {
        self.params().max_transfer_metadata_size
    }

    pub fn max_text_content_size(&self) -> usize {
        self.params().max_text_content_size
    }

    /// Does basic verification of the transaction.
    pub fn verify_transaction_basic(&self, p: &UnverifiedTransaction, _header: &Header) -> Result<(), Error> {
        let min_cost = self.min_cost(&p.action);
        if p.fee < min_cost {
            return Err(StateError::Parcel(ParcelError::InsufficientFee {
                minimal: min_cost,
                got: p.fee,
            })
            .into())
        }
        p.verify_basic(self.params()).map_err(StateError::from)?;

        Ok(())
    }

    /// Verify a particular transaction is valid, regardless of order.
    pub fn verify_transaction_unordered(
        &self,
        p: UnverifiedTransaction,
        _header: &Header,
    ) -> Result<SignedTransaction, Error> {
        p.check_low_s()?;
        Ok(SignedTransaction::try_new(p)?)
    }

    /// Does verification of the transaction against the parent state.
    pub fn verify_transaction<C: BlockInfo + TransactionInfo>(
        &self,
        tx: &SignedTransaction,
        header: &Header,
        client: &C,
        verify_timelock: bool,
    ) -> Result<(), Error> {
        if let Action::TransferAsset {
            inputs,
            orders,
            ..
        } = &tx.action
        {
            if verify_timelock {
                Self::verify_transfer_timelock(inputs, header, client)?;
            }
            Self::verify_transfer_order_expired(orders, header)?;
        }
        // FIXME: Filter transactions.
        Ok(())
    }

    /// Populate a header's fields based on its parent's header.
    /// Usually implements the chain scoring rule based on weight.
    pub fn populate_from_parent(&self, header: &mut Header, parent: &Header) {
        header.set_score(*parent.score());
    }

    fn verify_transfer_timelock<C: BlockInfo + TransactionInfo>(
        inputs: &[AssetTransferInput],
        header: &Header,
        client: &C,
    ) -> Result<(), Error> {
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
                        let absolute = client.transaction_block_number(&input.prev_out.tracker).ok_or_else(|| {
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
                            client.transaction_block_timestamp(&input.prev_out.tracker).ok_or_else(|| {
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

    fn verify_transfer_order_expired(orders: &[OrderOnTransfer], header: &Header) -> Result<(), Error> {
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

    fn min_cost(&self, action: &Action) -> u64 {
        match action {
            Action::MintAsset {
                ..
            } => self.params.min_asset_mint_cost,
            Action::TransferAsset {
                ..
            } => self.params.min_asset_transfer_cost,
            Action::ChangeAssetScheme {
                ..
            } => self.params.min_asset_scheme_change_cost,
            Action::ComposeAsset {
                ..
            } => self.params.min_asset_compose_cost,
            Action::DecomposeAsset {
                ..
            } => self.params.min_asset_decompose_cost,
            Action::UnwrapCCC {
                ..
            } => self.params.min_asset_unwrap_ccc_cost,
            Action::Pay {
                ..
            } => self.params.min_pay_transaction_cost,
            Action::SetRegularKey {
                ..
            } => self.params.min_set_regular_key_tranasction_cost,
            Action::CreateShard => self.params.min_create_shard_transaction_cost,
            Action::SetShardOwners {
                ..
            } => self.params.min_set_shard_owners_transaction_cost,
            Action::SetShardUsers {
                ..
            } => self.params.min_set_shard_users_transaction_cost,
            Action::WrapCCC {
                ..
            } => self.params.min_wrap_ccc_transaction_cost,
            Action::Custom {
                ..
            } => self.params.min_custom_transaction_cost,
            Action::Store {
                ..
            } => self.params.min_store_transaction_cost,
            Action::Remove {
                ..
            } => self.params.min_remove_transaction_cost,
        }
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

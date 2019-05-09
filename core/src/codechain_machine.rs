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
use ctypes::errors::{HistoryError, SyntaxError};
use ctypes::machine::{Machine, WithBalances};
use ctypes::transaction::{Action, AssetTransferInput, OrderOnTransfer, Timelock};
use ctypes::BlockNumber;

use crate::block::{ExecutedBlock, IsBlock};
use crate::client::{BlockInfo, TransactionInfo};
use crate::error::Error;
use crate::header::Header;
use crate::scheme::CommonParams;
use crate::transaction::{SignedTransaction, UnverifiedTransaction};

pub struct CodeChainMachine {
    params: CommonParams,
    is_order_disabled: bool,
}

impl CodeChainMachine {
    pub fn new(params: CommonParams) -> Self {
        CodeChainMachine {
            params,
            is_order_disabled: is_order_disabled(),
        }
    }

    /// Get the general parameters of the chain.
    pub fn common_params(&self, _block_number: Option<BlockNumber>) -> &CommonParams {
        &self.params
    }

    /// Does basic verification of the transaction.
    pub fn verify_transaction_basic(&self, p: &UnverifiedTransaction, header: &Header) -> Result<(), Error> {
        let min_cost = self.min_cost(&p.action, Some(header.number()));
        if p.fee < min_cost {
            return Err(SyntaxError::InsufficientFee {
                minimal: min_cost,
                got: p.fee,
            }
            .into())
        }
        p.verify_basic(self.common_params(Some(header.number())), self.is_order_disabled)?;

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
            expiration,
            ..
        } = &tx.action
        {
            Self::verify_transaction_expiration(&expiration, header)?;
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

    fn verify_transaction_expiration(expiration: &Option<u64>, header: &Header) -> Result<(), Error> {
        if expiration.is_none() {
            return Ok(())
        }
        let expiration = expiration.unwrap();

        if expiration < header.timestamp() {
            return Err(HistoryError::TransferExpired {
                expiration,
                timestamp: header.timestamp(),
            }
            .into())
        }
        Ok(())
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
                        return Err(HistoryError::Timelocked {
                            timelock,
                            remaining_time: value - header.number(),
                        }
                        .into())
                    }
                    Timelock::BlockAge(value) => {
                        let absolute = client.transaction_block_number(&input.prev_out.tracker).ok_or_else(|| {
                            Error::History(HistoryError::Timelocked {
                                timelock,
                                remaining_time: u64::max_value(),
                            })
                        })? + value;
                        if absolute > header.number() {
                            return Err(HistoryError::Timelocked {
                                timelock,
                                remaining_time: absolute - header.number(),
                            }
                            .into())
                        }
                    }
                    Timelock::Time(value) if value > header.timestamp() => {
                        return Err(HistoryError::Timelocked {
                            timelock,
                            remaining_time: value - header.timestamp(),
                        }
                        .into())
                    }
                    Timelock::TimeAge(value) => {
                        let absolute =
                            client.transaction_block_timestamp(&input.prev_out.tracker).ok_or_else(|| {
                                Error::History(HistoryError::Timelocked {
                                    timelock,
                                    remaining_time: u64::max_value(),
                                })
                            })? + value;
                        if absolute > header.timestamp() {
                            return Err(HistoryError::Timelocked {
                                timelock,
                                remaining_time: absolute - header.timestamp(),
                            }
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
                return Err(HistoryError::OrderExpired {
                    expiration: order_tx.order.expiration,
                    timestamp: header.timestamp(),
                }
                .into())
            }
        }
        Ok(())
    }

    pub fn min_cost(&self, action: &Action, block_number: Option<BlockNumber>) -> u64 {
        let params = self.common_params(block_number);
        match action {
            Action::MintAsset {
                ..
            } => params.min_asset_mint_cost,
            Action::TransferAsset {
                ..
            } => params.min_asset_transfer_cost,
            Action::ChangeAssetScheme {
                ..
            } => params.min_asset_scheme_change_cost,
            Action::IncreaseAssetSupply {
                ..
            } => params.min_asset_supply_increase_cost,
            Action::ComposeAsset {
                ..
            } => params.min_asset_compose_cost,
            Action::DecomposeAsset {
                ..
            } => params.min_asset_decompose_cost,
            Action::UnwrapCCC {
                ..
            } => params.min_asset_unwrap_ccc_cost,
            Action::Pay {
                ..
            } => params.min_pay_transaction_cost,
            Action::SetRegularKey {
                ..
            } => params.min_set_regular_key_transaction_cost,
            Action::CreateShard {
                ..
            } => params.min_create_shard_transaction_cost,
            Action::SetShardOwners {
                ..
            } => params.min_set_shard_owners_transaction_cost,
            Action::SetShardUsers {
                ..
            } => params.min_set_shard_users_transaction_cost,
            Action::WrapCCC {
                ..
            } => params.min_wrap_ccc_transaction_cost,
            Action::Custom {
                ..
            } => params.min_custom_transaction_cost,
            Action::Store {
                ..
            } => params.min_store_transaction_cost,
            Action::Remove {
                ..
            } => params.min_remove_transaction_cost,
        }
    }
}

fn is_order_disabled() -> bool {
    #[cfg(test)]
    const DEFAULT_ORDER_DISABLED: bool = false;
    #[cfg(not(test))]
    const DEFAULT_ORDER_DISABLED: bool = true;
    let var = std::env::var("ENABLE_ORDER");
    match var.as_ref().map(|x| x.trim()) {
        Ok(value) => !value.parse::<bool>().unwrap(),
        Err(std::env::VarError::NotPresent) => DEFAULT_ORDER_DISABLED,
        Err(err) => unreachable!("{:?}", err),
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

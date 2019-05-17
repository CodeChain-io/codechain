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

use std::iter::Iterator;

use ckey::Address;
use cstate::{StateError, TopState, TopStateView};
use ctypes::errors::{HistoryError, SyntaxError};
use ctypes::transaction::{Action, AssetTransferInput, OrderOnTransfer, Timelock};
use ctypes::{BlockNumber, CommonParams};

use crate::block::{ExecutedBlock, IsBlock};
use crate::client::BlockChainTrait;
use crate::error::Error;
use crate::header::Header;
use crate::transaction::{SignedTransaction, UnverifiedTransaction};

struct Params {
    changed_block: BlockNumber,
    params: CommonParams,
}

pub struct CodeChainMachine {
    params: Vec<Params>,
    is_order_disabled: bool,
}

impl CodeChainMachine {
    pub fn new(params: CommonParams) -> Self {
        CodeChainMachine {
            params: vec![Params {
                changed_block: 0,
                params,
            }],
            is_order_disabled: is_order_disabled(),
        }
    }

    /// Get the general parameters of the chain.
    pub fn common_params(&self, block_number: Option<BlockNumber>) -> CommonParams {
        let params = &self.params;
        assert!(!params.is_empty());
        let block_number = if let Some(block_number) = block_number {
            block_number
        } else {
            return params.last().unwrap().params // the latest block.
        };

        params
            .iter()
            .take_while(
                |Params {
                     changed_block,
                     ..
                 }| *changed_block <= block_number,
            )
            .last()
            .unwrap()
            .params
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
        p.verify_basic(&self.common_params(Some(header.number())), self.is_order_disabled)?;

        Ok(())
    }

    /// Verify a particular transaction's seal is valid.
    pub fn verify_transaction_seal(p: UnverifiedTransaction, _header: &Header) -> Result<SignedTransaction, Error> {
        p.check_low_s()?;
        Ok(SignedTransaction::try_new(p)?)
    }

    /// Does verification of the transaction against the parent state.
    pub fn verify_transaction<C: BlockChainTrait>(
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

    fn verify_transfer_timelock<C: BlockChainTrait>(
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
            } => params.min_asset_mint_cost(),
            Action::TransferAsset {
                ..
            } => params.min_asset_transfer_cost(),
            Action::ChangeAssetScheme {
                ..
            } => params.min_asset_scheme_change_cost(),
            Action::IncreaseAssetSupply {
                ..
            } => params.min_asset_supply_increase_cost(),
            Action::ComposeAsset {
                ..
            } => params.min_asset_compose_cost(),
            Action::DecomposeAsset {
                ..
            } => params.min_asset_decompose_cost(),
            Action::UnwrapCCC {
                ..
            } => params.min_asset_unwrap_ccc_cost(),
            Action::Pay {
                ..
            } => params.min_pay_transaction_cost(),
            Action::SetRegularKey {
                ..
            } => params.min_set_regular_key_transaction_cost(),
            Action::CreateShard {
                ..
            } => params.min_create_shard_transaction_cost(),
            Action::SetShardOwners {
                ..
            } => params.min_set_shard_owners_transaction_cost(),
            Action::SetShardUsers {
                ..
            } => params.min_set_shard_users_transaction_cost(),
            Action::WrapCCC {
                ..
            } => params.min_wrap_ccc_transaction_cost(),
            Action::Custom {
                ..
            } => params.min_custom_transaction_cost(),
            Action::Store {
                ..
            } => params.min_store_transaction_cost(),
            Action::Remove {
                ..
            } => params.min_remove_transaction_cost(),
        }
    }

    pub fn balance(&self, live: &ExecutedBlock, address: &Address) -> Result<u64, Error> {
        Ok(live.state().balance(address).map_err(StateError::from)?)
    }

    pub fn add_balance(&self, live: &mut ExecutedBlock, address: &Address, amount: u64) -> Result<(), Error> {
        live.state_mut().add_balance(address, amount).map_err(StateError::from)?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_params_are_not_changed_since_genesis() {
        let genesis_params = CommonParams::default_for_test();
        let machine = CodeChainMachine::new(genesis_params);
        assert_eq!(genesis_params, machine.common_params(Some(0)));
        assert_eq!(genesis_params, machine.common_params(Some(1)));
        assert_eq!(genesis_params, machine.common_params(None));
    }

    #[test]
    fn common_params_changed_at_1() {
        let genesis_params = CommonParams::default_for_test();
        let params_at_1 = {
            let mut params = genesis_params;
            params.set_min_store_transaction_cost(genesis_params.min_store_transaction_cost() + 10);
            params
        };
        let machine = CodeChainMachine {
            params: vec![
                Params {
                    changed_block: 0,
                    params: genesis_params,
                },
                Params {
                    changed_block: 1,
                    params: params_at_1,
                },
            ],
            is_order_disabled: false,
        };
        assert_eq!(genesis_params, machine.common_params(Some(0)));
        assert_eq!(params_at_1, machine.common_params(Some(1)));
        assert_eq!(params_at_1, machine.common_params(None));
    }

    #[test]
    fn common_params_changed_at_2() {
        let genesis_params = CommonParams::default_for_test();
        let params_at_2 = {
            let mut params = genesis_params;
            params.set_min_store_transaction_cost(genesis_params.min_store_transaction_cost() + 10);
            params
        };
        let machine = CodeChainMachine {
            params: vec![
                Params {
                    changed_block: 0,
                    params: genesis_params,
                },
                Params {
                    changed_block: 2,
                    params: params_at_2,
                },
            ],
            is_order_disabled: false,
        };
        assert_eq!(genesis_params, machine.common_params(Some(0)));
        assert_eq!(genesis_params, machine.common_params(Some(1)));
        assert_eq!(params_at_2, machine.common_params(Some(2)));
        assert_eq!(params_at_2, machine.common_params(None));
    }


    #[test]
    fn common_params_changed_several_times() {
        let genesis_params = CommonParams::default_for_test();
        let params_at_10 = {
            let mut params = genesis_params;
            params.set_min_store_transaction_cost(genesis_params.min_store_transaction_cost() + 10);
            params
        };
        let params_at_20 = {
            let mut params = params_at_10;
            params.set_min_store_transaction_cost(params_at_10.min_store_transaction_cost() + 10);
            params
        };
        let params_at_30 = {
            let mut params = params_at_20;
            params.set_min_store_transaction_cost(params_at_20.min_store_transaction_cost() + 10);
            params
        };
        let machine = CodeChainMachine {
            params: vec![
                Params {
                    changed_block: 0,
                    params: genesis_params,
                },
                Params {
                    changed_block: 10,
                    params: params_at_10,
                },
                Params {
                    changed_block: 20,
                    params: params_at_20,
                },
                Params {
                    changed_block: 30,
                    params: params_at_30,
                },
            ],
            is_order_disabled: false,
        };
        for i in 0..10 {
            assert_eq!(genesis_params, machine.common_params(Some(i)), "unexpected params at block {}", i);
        }
        for i in 10..20 {
            assert_eq!(params_at_10, machine.common_params(Some(i)), "unexpected params at block {}", i);
        }
        for i in 20..30 {
            assert_eq!(params_at_20, machine.common_params(Some(i)), "unexpected params at block {}", i);
        }
        for i in 30..40 {
            assert_eq!(params_at_30, machine.common_params(Some(i)), "unexpected params at block {}", i);
        }
        assert_eq!(params_at_30, machine.common_params(None));
    }
}

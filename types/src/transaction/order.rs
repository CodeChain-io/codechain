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

use primitives::{Bytes, H160};

use crate::ShardId;

use super::error::Error;
use super::{AssetOutPoint, AssetTransferOutput};

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable)]
pub struct Order {
    // Main order information
    pub asset_type_from: H160,
    pub asset_type_to: H160,
    pub asset_type_fee: H160,
    pub shard_id_from: ShardId,
    pub shard_id_to: ShardId,
    pub shard_id_fee: ShardId,
    pub asset_quantity_from: u64,
    pub asset_quantity_to: u64,
    pub asset_quantity_fee: u64,
    /// previous outputs that order is started
    pub origin_outputs: Vec<AssetOutPoint>,
    /// expiration time by second
    pub expiration: u64,
    pub lock_script_hash_from: H160,
    pub parameters_from: Vec<Bytes>,
    pub lock_script_hash_fee: H160,
    pub parameters_fee: Vec<Bytes>,
}

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable)]
pub struct OrderOnTransfer {
    pub order: Order,
    /// Spent quantity of asset_type_from
    pub spent_quantity: u64,
    /// Indices of transfer inputs which are moved as order
    pub input_indices: Vec<usize>,
    /// Indices of transfer outputs which are moved as order
    pub output_indices: Vec<usize>,
}

impl Order {
    // FIXME: Remove this after the clippy nonminimal bool bug is fixed
    // https://rust-lang.github.io/rust-clippy/v0.0.212/#nonminimal_bool
    #![cfg_attr(feature = "cargo-clippy", allow(clippy::nonminimal_bool))]
    pub fn verify(&self) -> Result<(), Error> {
        // If asset_quantity_fee is zero, it means there's no fee to pay.
        if (self.asset_type_from == self.asset_type_to && self.shard_id_from == self.shard_id_to)
            || self.asset_quantity_fee != 0
                && ((self.asset_type_from == self.asset_type_fee && self.shard_id_from == self.shard_id_fee)
                    || (self.asset_type_to == self.asset_type_fee && self.shard_id_to == self.shard_id_fee))
        {
            return Err(Error::InvalidOrderAssetTypes)
        }
        if (self.asset_quantity_from == 0) ^ (self.asset_quantity_to == 0) {
            return Err(Error::InvalidOrderAssetQuantities {
                from: self.asset_quantity_from,
                to: self.asset_quantity_to,
                fee: self.asset_quantity_fee,
            })
        }
        if self.asset_quantity_from == 0 && self.asset_quantity_fee != 0
            || self.asset_quantity_from != 0 && self.asset_quantity_fee % self.asset_quantity_from != 0
        {
            return Err(Error::InvalidOrderAssetQuantities {
                from: self.asset_quantity_from,
                to: self.asset_quantity_to,
                fee: self.asset_quantity_fee,
            })
        }
        if self.asset_quantity_fee != 0
            && self.lock_script_hash_fee == self.lock_script_hash_from
            && self.parameters_fee == self.parameters_from
        {
            return Err(Error::OrderRecipientsAreSame)
        }
        if self.origin_outputs.is_empty() {
            return Err(Error::InvalidOriginOutputs(self.hash()))
        }
        for origin_output in self.origin_outputs.iter() {
            if (origin_output.asset_type != self.asset_type_from || origin_output.shard_id != self.shard_id_from)
                && (origin_output.asset_type != self.asset_type_fee || origin_output.shard_id != self.shard_id_fee)
            {
                return Err(Error::InvalidOriginOutputs(self.hash()))
            }
        }
        Ok(())
    }

    pub fn check_transfer_output(&self, output: &AssetTransferOutput) -> Result<bool, Error> {
        if self.asset_quantity_fee != 0
            && self.asset_type_fee == output.asset_type
            && self.shard_id_fee == output.shard_id
            && self.lock_script_hash_fee == output.lock_script_hash
            && self.parameters_fee == output.parameters
        {
            // owned by relayer
            return Ok(false)
        }

        if self.lock_script_hash_from != output.lock_script_hash {
            return Err(Error::InvalidOrderLockScriptHash(self.lock_script_hash_from))
        }
        if self.parameters_from != output.parameters {
            return Err(Error::InvalidOrderParameters(self.parameters_from.to_vec()))
        }
        // owned by maker
        Ok(true)
    }

    pub fn consume(&self, quantity: u64) -> Order {
        Order {
            asset_type_from: self.asset_type_from,
            asset_type_to: self.asset_type_to,
            asset_type_fee: self.asset_type_fee,
            shard_id_from: self.shard_id_from,
            shard_id_to: self.shard_id_to,
            shard_id_fee: self.shard_id_fee,
            asset_quantity_from: self.asset_quantity_from - quantity,
            asset_quantity_to: self.asset_quantity_to
                - (u128::from(quantity) * u128::from(self.asset_quantity_to) / u128::from(self.asset_quantity_from))
                    as u64,
            asset_quantity_fee: self.asset_quantity_fee
                - (u128::from(quantity) * u128::from(self.asset_quantity_fee) / u128::from(self.asset_quantity_from))
                    as u64,
            origin_outputs: self.origin_outputs.clone(),
            expiration: self.expiration,
            lock_script_hash_from: self.lock_script_hash_from,
            parameters_from: self.parameters_from.clone(),
            lock_script_hash_fee: self.lock_script_hash_fee,
            parameters_fee: self.parameters_fee.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::H256;

    #[test]
    fn verify_order_success() {
        let asset_type_from = H160::random();
        let asset_type_to = H160::random();
        let asset_type_fee = H160::random();
        let order = Order {
            asset_type_from,
            asset_type_to,
            asset_type_fee,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 3,
            asset_quantity_to: 2,
            asset_quantity_fee: 3,
            origin_outputs: vec![AssetOutPoint {
                tracker: H256::random(),
                index: 0,
                asset_type: asset_type_from,
                shard_id: 0,
                quantity: 10,
            }],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(order.verify(), Ok(()));

        let order = Order {
            asset_type_from,
            asset_type_to,
            asset_type_fee,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 3,
            asset_quantity_to: 2,
            asset_quantity_fee: 0,
            origin_outputs: vec![AssetOutPoint {
                tracker: H256::random(),
                index: 0,
                asset_type: asset_type_from,
                shard_id: 0,
                quantity: 10,
            }],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(order.verify(), Ok(()));

        let order = Order {
            asset_type_from,
            asset_type_to,
            asset_type_fee,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 0,
            asset_quantity_to: 0,
            asset_quantity_fee: 0,
            origin_outputs: vec![AssetOutPoint {
                tracker: H256::random(),
                index: 0,
                asset_type: asset_type_from,
                shard_id: 0,
                quantity: 10,
            }],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(order.verify(), Ok(()));
    }

    #[test]
    fn verify_order_fail() {
        // 1. origin outputs are invalid
        let asset_type_from = H160::random();
        let asset_type_to = H160::random();
        let asset_type_fee = H160::random();
        let order = Order {
            asset_type_from,
            asset_type_to,
            asset_type_fee,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 3,
            asset_quantity_to: 2,
            asset_quantity_fee: 3,
            origin_outputs: vec![AssetOutPoint {
                tracker: H256::random(),
                index: 0,
                asset_type: H160::random(),
                shard_id: 0,
                quantity: 10,
            }],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(order.verify(), Err(Error::InvalidOriginOutputs(order.hash())));

        let order = Order {
            asset_type_from,
            asset_type_to,
            asset_type_fee,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 3,
            asset_quantity_to: 2,
            asset_quantity_fee: 3,
            origin_outputs: vec![],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(order.verify(), Err(Error::InvalidOriginOutputs(order.hash())));

        // 2. asset quantitys are invalid
        let order = Order {
            asset_type_from,
            asset_type_to,
            asset_type_fee,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 3,
            asset_quantity_to: 0,
            asset_quantity_fee: 3,
            origin_outputs: vec![AssetOutPoint {
                tracker: H256::random(),
                index: 0,
                asset_type: asset_type_from,
                shard_id: 0,
                quantity: 10,
            }],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(
            order.verify(),
            Err(Error::InvalidOrderAssetQuantities {
                from: 3,
                to: 0,
                fee: 3,
            })
        );

        let order = Order {
            asset_type_from,
            asset_type_to,
            asset_type_fee,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 0,
            asset_quantity_to: 2,
            asset_quantity_fee: 3,
            origin_outputs: vec![AssetOutPoint {
                tracker: H256::random(),
                index: 0,
                asset_type: asset_type_from,
                shard_id: 0,
                quantity: 10,
            }],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(
            order.verify(),
            Err(Error::InvalidOrderAssetQuantities {
                from: 0,
                to: 2,
                fee: 3,
            })
        );

        let order = Order {
            asset_type_from,
            asset_type_to,
            asset_type_fee,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 0,
            asset_quantity_to: 0,
            asset_quantity_fee: 3,
            origin_outputs: vec![AssetOutPoint {
                tracker: H256::random(),
                index: 0,
                asset_type: asset_type_from,
                shard_id: 0,
                quantity: 10,
            }],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(
            order.verify(),
            Err(Error::InvalidOrderAssetQuantities {
                from: 0,
                to: 0,
                fee: 3,
            })
        );

        let order = Order {
            asset_type_from,
            asset_type_to,
            asset_type_fee,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 3,
            asset_quantity_to: 2,
            asset_quantity_fee: 2,
            origin_outputs: vec![AssetOutPoint {
                tracker: H256::random(),
                index: 0,
                asset_type: asset_type_from,
                shard_id: 0,
                quantity: 10,
            }],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(
            order.verify(),
            Err(Error::InvalidOrderAssetQuantities {
                from: 3,
                to: 2,
                fee: 2,
            })
        );

        // 3. asset types are same
        let asset_type = H160::random();
        let order = Order {
            asset_type_from: asset_type,
            asset_type_to: asset_type,
            asset_type_fee,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 3,
            asset_quantity_to: 2,
            asset_quantity_fee: 3,
            origin_outputs: vec![AssetOutPoint {
                tracker: H256::random(),
                index: 0,
                asset_type,
                shard_id: 0,
                quantity: 10,
            }],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(order.verify(), Err(Error::InvalidOrderAssetTypes));

        let asset_type = H160::random();
        let order = Order {
            asset_type_from: asset_type,
            asset_type_to,
            asset_type_fee: asset_type,
            shard_id_from: 0,
            shard_id_to: 0,
            shard_id_fee: 0,
            asset_quantity_from: 3,
            asset_quantity_to: 2,
            asset_quantity_fee: 3,
            origin_outputs: vec![AssetOutPoint {
                tracker: H256::random(),
                index: 0,
                asset_type,
                shard_id: 0,
                quantity: 10,
            }],
            expiration: 10,
            lock_script_hash_from: H160::random(),
            parameters_from: vec![vec![1]],
            lock_script_hash_fee: H160::random(),
            parameters_fee: vec![vec![1]],
        };
        assert_eq!(order.verify(), Err(Error::InvalidOrderAssetTypes));
    }
}

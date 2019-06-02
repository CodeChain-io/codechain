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

use std::convert::{TryFrom, TryInto};

use cjson::uint::Uint;
use ctypes::transaction::{Order as OrderType, OrderOnTransfer as OrderOnTransferType};
use ctypes::ShardId;
use primitives::H160;
use rustc_serialize::hex::{FromHex, FromHexError, ToHex};

use super::AssetOutPoint;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub asset_type_from: H160,
    pub asset_type_to: H160,
    pub asset_type_fee: H160,
    pub shard_id_from: ShardId,
    pub shard_id_to: ShardId,
    pub shard_id_fee: ShardId,
    pub asset_quantity_from: Uint,
    pub asset_quantity_to: Uint,
    pub asset_quantity_fee: Uint,
    pub origin_outputs: Vec<AssetOutPoint>,
    pub expiration: Uint,
    pub lock_script_hash_from: H160,
    pub parameters_from: Vec<String>,
    pub lock_script_hash_fee: H160,
    pub parameters_fee: Vec<String>,
}

impl From<OrderType> for Order {
    fn from(from: OrderType) -> Self {
        Order {
            asset_type_from: from.asset_type_from,
            asset_type_to: from.asset_type_to,
            asset_type_fee: from.asset_type_fee,
            shard_id_from: from.shard_id_from,
            shard_id_to: from.shard_id_to,
            shard_id_fee: from.shard_id_fee,
            asset_quantity_from: from.asset_quantity_from.into(),
            asset_quantity_to: from.asset_quantity_to.into(),
            asset_quantity_fee: from.asset_quantity_fee.into(),
            origin_outputs: from.origin_outputs.into_iter().map(From::from).collect(),
            expiration: from.expiration.into(),
            lock_script_hash_from: from.lock_script_hash_from,
            parameters_from: from.parameters_from.into_iter().map(|param| param.to_hex()).collect(),
            lock_script_hash_fee: from.lock_script_hash_fee,
            parameters_fee: from.parameters_fee.into_iter().map(|param| param.to_hex()).collect(),
        }
    }
}

impl TryFrom<Order> for OrderType {
    type Error = FromHexError;
    fn try_from(from: Order) -> Result<Self, Self::Error> {
        let parameters_from =
            from.parameters_from.into_iter().map(|param| param.from_hex()).collect::<Result<_, _>>()?;
        let parameters_fee = from.parameters_fee.into_iter().map(|param| param.from_hex()).collect::<Result<_, _>>()?;
        Ok(OrderType {
            asset_type_from: from.asset_type_from,
            asset_type_to: from.asset_type_to,
            asset_type_fee: from.asset_type_fee,
            shard_id_from: from.shard_id_from,
            shard_id_to: from.shard_id_to,
            shard_id_fee: from.shard_id_fee,
            asset_quantity_from: from.asset_quantity_from.into(),
            asset_quantity_to: from.asset_quantity_to.into(),
            asset_quantity_fee: from.asset_quantity_fee.into(),
            origin_outputs: from.origin_outputs.into_iter().map(From::from).collect(),
            expiration: from.expiration.into(),
            lock_script_hash_from: from.lock_script_hash_from,
            parameters_from,
            lock_script_hash_fee: from.lock_script_hash_fee,
            parameters_fee,
        })
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderOnTransfer {
    pub order: Order,
    /// Spent quantity of asset_type_from
    pub spent_quantity: Uint,
    // Indices of asset_type_from
    pub input_from_indices: Vec<usize>,
    // Indices of asset_type_fee
    pub input_fee_indices: Vec<usize>,
    // Indices of remain asset_type_from
    pub output_from_indices: Vec<usize>,
    // Indices of asset_type_to
    pub output_to_indices: Vec<usize>,
    // Indices of ramain asset_type_fee
    pub output_owned_fee_indices: Vec<usize>,
    // Indices of paid asset_type_fee
    pub output_transferred_fee_indices: Vec<usize>,
}

impl From<OrderOnTransferType> for OrderOnTransfer {
    fn from(from: OrderOnTransferType) -> Self {
        OrderOnTransfer {
            order: from.order.into(),
            spent_quantity: from.spent_quantity.into(),
            input_from_indices: from.input_from_indices,
            input_fee_indices: from.input_fee_indices,
            output_from_indices: from.output_from_indices,
            output_to_indices: from.output_to_indices,
            output_owned_fee_indices: from.output_owned_fee_indices,
            output_transferred_fee_indices: from.output_transferred_fee_indices,
        }
    }
}

impl TryFrom<OrderOnTransfer> for OrderOnTransferType {
    type Error = FromHexError;
    fn try_from(from: OrderOnTransfer) -> Result<Self, Self::Error> {
        Ok(OrderOnTransferType {
            order: from.order.try_into()?,
            spent_quantity: from.spent_quantity.into(),
            input_from_indices: from.input_from_indices,
            input_fee_indices: from.input_fee_indices,
            output_from_indices: from.output_from_indices,
            output_to_indices: from.output_to_indices,
            output_owned_fee_indices: from.output_owned_fee_indices,
            output_transferred_fee_indices: from.output_transferred_fee_indices,
        })
    }
}

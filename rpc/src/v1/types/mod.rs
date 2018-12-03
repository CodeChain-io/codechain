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

mod action;
mod asset_input;
mod asset_output;
mod block;
mod order;
mod text;
mod transaction;
mod unsigned_transaction;
mod work;

use primitives::H256;

use self::asset_input::{AssetOutPoint, AssetTransferInput};
use self::asset_output::{AssetMintOutput, AssetTransferOutput};
use self::order::OrderOnTransfer;

pub use self::action::{Action, ActionWithTracker};
pub use self::block::Block;
pub use self::block::BlockNumberAndHash;
pub use self::text::Text;
pub use self::transaction::Transaction;
pub use self::unsigned_transaction::UnsignedTransaction;
pub use self::work::Work;

use serde::de::{self, Deserialize, Deserializer};

#[derive(Debug, Serialize, Deserialize)]
pub struct FilterStatus {
    pub list: Vec<(::std::net::IpAddr, String)>,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendTransactionResult {
    pub hash: H256,
    pub seq: u64,
}

#[derive(Debug)]
pub enum TPSTestOption {
    PayOnly,
    TransferSingle,
    TransferMultiple,
    PayOrTransfer,
}

impl<'de> Deserialize<'de> for TPSTestOption {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>, {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "payOnly" => TPSTestOption::PayOnly,
            "transferSingle" => TPSTestOption::TransferSingle,
            "transferMultiple" => TPSTestOption::TransferMultiple,
            "payOrTransfer" => TPSTestOption::PayOrTransfer,
            v => Err(de::Error::custom(format!(
                "Invalid params: unknown variant `{}`, expected one of \
                 `payOnly`, `transferSingle`, `transferMultiple`, `payOrTransfer`.",
                v
            )))?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TPSTestSetting {
    pub count: u64,
    pub seed: u64,
    pub option: TPSTestOption,
}

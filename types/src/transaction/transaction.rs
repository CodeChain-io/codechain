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

use super::Action;
use super::{AssetWrapCCCOutput, ShardTransaction};
use crate::{Tracker, TxHash};
use ccrypto::blake256;
use ckey::NetworkId;
use rlp::RlpStream;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Seq.
    pub seq: u64,
    /// Quantity of CCC to be paid as a cost for distributing this transaction to the network.
    pub fee: u64,
    /// Network Id
    pub network_id: NetworkId,

    pub action: Action,
}

impl Transaction {
    /// Append object with a without signature into RLP stream
    pub fn rlp_append_unsigned(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&self.seq);
        s.append(&self.fee);
        s.append(&self.network_id);
        s.append(&self.action);
    }

    /// The message hash of the transaction.
    pub fn hash(&self) -> TxHash {
        let mut stream = RlpStream::new();
        self.rlp_append_unsigned(&mut stream);
        blake256(stream.as_raw()).into()
    }

    pub fn tracker(&self) -> Option<Tracker> {
        let shard_tx = match self.action.clone() {
            Action::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                quantity,
                ..
            } => Some(ShardTransaction::WrapCCC {
                network_id: self.network_id,
                shard_id,
                tx_hash: self.hash(),
                output: AssetWrapCCCOutput {
                    lock_script_hash,
                    parameters,
                    quantity,
                },
            }),
            other_actions => other_actions.into(),
        };
        shard_tx.map(|t| t.tracker())
    }
    pub fn is_master_key_allowed(&self) -> bool {
        match self.action {
            Action::SetRegularKey {
                ..
            } => true,
            _ => false,
        }
    }
}

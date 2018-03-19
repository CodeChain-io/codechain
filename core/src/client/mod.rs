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

mod chain_notify;
mod client;
mod error;

pub use self::chain_notify::ChainNotify;
pub use self::client::Client;
pub use self::error::Error;

use cbytes::Bytes;
use ctypes::H256;

use super::blockchain_info::BlockChainInfo;

/// Provides `chain_info` method
pub trait ChainInfo {
    /// Get blockchain information.
    fn chain_info(&self) -> BlockChainInfo;
}

/// Client facilities used by internally sealing Engines.
pub trait EngineClient: Sync + Send  + ChainInfo {
    /// Broadcast a consensus message to the network.
    fn broadcast_consensus_message(&self, message: Bytes);

    /// Make a new block and seal it.
    fn update_sealing(&self);

    /// Submit a seal for a block in the mining queue.
    fn submit_seal(&self, block_hash: H256, seal: Vec<Bytes>);
}


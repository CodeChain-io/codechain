// Copyright 2019 Kodebox, Inc.
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

use std::sync::Arc;

use ccore::ibc;
use ccore::{BlockChainClient, BlockId, StateInfo};
use jsonrpc_core::Result;
use primitives::Bytes;
use rustc_serialize::hex::ToHex;

use super::super::errors;
use super::super::traits::IBC;
use super::super::types::IBCQueryResult;

pub struct IBCClient<C>
where
    C: StateInfo + BlockChainClient, {
    client: Arc<C>,
}

impl<C> IBCClient<C>
where
    C: StateInfo + BlockChainClient,
{
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
        }
    }
}

impl<C> IBC for IBCClient<C>
where
    C: StateInfo + 'static + Send + Sync + BlockChainClient,
{
    fn query_client_consensus_state(
        &self,
        client_id: String,
        block_number: Option<u64>,
    ) -> Result<Option<IBCQueryResult>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        let mut state = self.client.state_at(block_id).ok_or_else(errors::state_not_exist)?;
        let block_number = match self.client.block_number(&block_id) {
            None => return Ok(None),
            Some(block_number) => block_number,
        };

        let mut context = ibc::context::TopLevelContext::new(&mut state);
        let client_manager = ibc::client::Manager::new();
        let client_state =
            client_manager.query(&mut context, &client_id).map_err(|_| errors::ibc_client_not_exist())?;

        let consensus_state = client_state.get_consensus_state(&mut context);

        let rlp_encoded_consensus_state = consensus_state.encode();

        Ok(Some(IBCQueryResult {
            block_number,
            raw: rlp_encoded_consensus_state.to_hex(),
            // FIXME
            proof: "".to_string(),
        }))
    }

    fn query_header(&self, block_number: Option<u64>) -> Result<Option<String>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        let header = match self.client.block_header(&block_id) {
            None => return Ok(None),
            Some(header) => header,
        };

        Ok(Some(header.into_inner().to_hex()))
    }

    fn query_client_root(
        &self,
        client_id: String,
        other_block_number: u64,
        this_block_number: Option<u64>,
    ) -> Result<Option<IBCQueryResult>> {
        let block_id = this_block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        let mut state = self.client.state_at(block_id).ok_or_else(errors::state_not_exist)?;
        let block_number = match self.client.block_number(&block_id) {
            None => return Ok(None),
            Some(block_number) => block_number,
        };

        let mut context = ibc::context::TopLevelContext::new(&mut state);
        let client_manager = ibc::client::Manager::new();
        let client_state =
            client_manager.query(&mut context, &client_id).map_err(|_| errors::ibc_client_not_exist())?;

        let root =
            client_state.get_root(&mut context, other_block_number).map_err(|_| errors::ibc_client_root_not_exist())?;
        let rlp_encoded_root = root.encode();

        Ok(Some(IBCQueryResult {
            block_number,
            raw: rlp_encoded_root.to_hex(),
            // FIXME
            proof: "".to_string(),
        }))
    }
}

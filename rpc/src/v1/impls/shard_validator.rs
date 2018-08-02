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

use std::sync::Arc;

use ccore::ShardValidatorClient as CoreClient;
use ckey::SignatureData;
use ctypes::parcel::Action;
use jsonrpc_core::Result;
use primitives::H256;

use super::super::ShardValidator;

#[allow(dead_code)]
pub struct ShardValidatorClient<C>
where
    C: CoreClient, {
    client: Arc<C>,
}

impl<C> ShardValidatorClient<C>
where
    C: CoreClient,
{
    pub fn new(client: Arc<C>) -> ShardValidatorClient<C> {
        Self {
            client,
        }
    }
}

impl<C> ShardValidator for ShardValidatorClient<C>
where
    C: CoreClient + 'static,
{
    fn get_signatures(&self, action_hash: H256) -> Result<Vec<SignatureData>> {
        Ok(self.client.signatures(&action_hash).into_iter().map(SignatureData::from).collect())
    }

    fn register_action(&self, action: Action) -> Result<bool> {
        Ok(self.client.register_action(action))
    }
}

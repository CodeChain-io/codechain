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

use ccore::{AccountProvider, Client, Miner};
use cnetwork::NetworkControl;
use crpc::{MetaIoHandler, Params, Value};

pub struct ApiDependencies<NC>
where
    NC: 'static + NetworkControl + Send + Sync, {
    pub client: Arc<Client>,
    pub miner: Arc<Miner>,
    pub network_control: Option<Arc<NC>>,
    pub account_provider: Arc<AccountProvider>,
}

impl<NC> ApiDependencies<NC>
where
    NC: 'static + NetworkControl + Send + Sync,
{
    pub fn extend_api(&self, handler: &mut MetaIoHandler<()>) {
        use crpc::v1::*;
        handler.extend_with(ChainClient::new(&self.client, &self.miner).to_delegate());
        handler.extend_with(DevelClient::new(&self.client).to_delegate());
        handler.extend_with(MinerClient::new(&self.client, &self.miner).to_delegate());
        handler.extend_with(NetClient::new(&self.network_control).to_delegate());
        handler.extend_with(AccountClient::new(&self.account_provider).to_delegate());
    }
}

pub fn setup_rpc(mut handler: MetaIoHandler<()>) -> MetaIoHandler<()> {
    handler.add_method("ping", |_params: Params| Ok(Value::String("pong".to_string())));
    handler.add_method("version", |_params: Params| Ok(Value::String(env!("CARGO_PKG_VERSION").to_string())));
    handler
}

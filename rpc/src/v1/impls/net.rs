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

use cnetwork::{NetworkControl, SocketAddr};
use jsonrpc_core::Result;
use primitives::H256;

use super::super::errors;
use super::super::traits::Net;

pub struct NetClient<NC>
where
    NC: NetworkControl + Send + Sync, {
    network_control: Option<Arc<NC>>,
}

impl<NC> NetClient<NC>
where
    NC: NetworkControl + Send + Sync,
{
    pub fn new(network_control: &Option<Arc<NC>>) -> Self {
        Self {
            network_control: network_control.as_ref().map(Arc::clone),
        }
    }
}

impl<NC> Net for NetClient<NC>
where
    NC: 'static + NetworkControl + Send + Sync,
{
    fn share_secret(&self, secret: H256, address: ::std::net::IpAddr, port: u16) -> Result<()> {
        let network_control = self.network_control.as_ref().ok_or_else(|| errors::network_disabled())?;
        network_control.register_secret(secret, SocketAddr::new(address, port));
        Ok(())
    }

    fn connect(&self, address: ::std::net::IpAddr, port: u16) -> Result<()> {
        let network_control = self.network_control.as_ref().ok_or_else(|| errors::network_disabled())?;
        network_control.connect(SocketAddr::new(address, port));
        Ok(())
    }

    fn disconnect(&self, address: ::std::net::IpAddr, port: u16) -> Result<()> {
        let network_control = self.network_control.as_ref().ok_or_else(|| errors::network_disabled())?;
        network_control.disconnect(SocketAddr::new(address, port)).map_err(errors::network_control)?;
        Ok(())
    }

    fn is_connected(&self, address: ::std::net::IpAddr, port: u16) -> Result<bool> {
        let network_control = self.network_control.as_ref().ok_or_else(errors::network_disabled)?;
        Ok(network_control.is_connected(&SocketAddr::new(address, port)))
    }
}

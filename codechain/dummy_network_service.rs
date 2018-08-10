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

use cnetwork::{NetworkControl, NetworkControlError, SocketAddr};
use primitives::H256;

pub struct DummyNetworkService {}

impl DummyNetworkService {
    pub fn new() -> Self {
        DummyNetworkService {}
    }
}

impl NetworkControl for DummyNetworkService {
    fn register_secret(&self, _secret: H256, _addr: SocketAddr) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn connect(&self, _addr: SocketAddr) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn disconnect(&self, _addr: SocketAddr) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn is_connected(&self, _addr: &SocketAddr) -> Result<bool, NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn get_port(&self) -> Result<u16, NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn get_peer_count(&self) -> Result<usize, NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }
}

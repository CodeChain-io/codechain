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

use super::ClientApi;
use super::Error;
use super::NodeId;

pub trait Extension {
    fn name(&self) -> String;
    fn need_encryption(&self) -> bool;

    fn on_initialize(&self, api: Arc<ClientApi>);

    fn on_node_added(&self, id: &NodeId);
    fn on_node_removed(&self, id: &NodeId);

    fn on_connected(&self, id: &NodeId);
    fn on_connection_allowed(&self, id: &NodeId);
    fn on_connection_denied(&self, id: &NodeId, error: Error);

    fn on_message(&self, id: &NodeId, message: &Vec<u8>);

    fn on_close(&self);
}

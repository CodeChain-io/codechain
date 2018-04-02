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

use std::result;
use std::sync::Arc;

use cio::{StreamToken};

pub type NodeId = StreamToken;

#[derive(Clone, Copy, Debug)]
pub enum Error {
    ExtensionDropped,
    DuplicatedTimerId,
    NoMoreTimerToken,
}

pub type Result<T> = result::Result<T, Error>;

pub trait Api: Send + Sync {
    fn send(&self, id: &NodeId, message: &Vec<u8>);
    fn connect(&self, id: &NodeId);

    fn set_timer(&self, timer_id: usize, ms: u64);
    fn set_timer_once(&self, timer_id: usize, ms: u64);
    fn clear_timer(&self, timer_id: usize);
}

pub trait Extension: Send + Sync {
    fn name(&self) -> String;
    fn need_encryption(&self) -> bool;

    fn on_initialize(&self, api: Arc<Api>);

    fn on_node_added(&self, _id: &NodeId) {}
    fn on_node_removed(&self, _id: &NodeId) {}

    fn on_connected(&self, _id: &NodeId) {}
    fn on_connection_allowed(&self, _id: &NodeId) {}
    fn on_connection_denied(&self, _id: &NodeId, _error: Error) {}

    fn on_message(&self, _id: &NodeId, _message: &Vec<u8>) {}

    fn on_close(&self) {}

    fn on_timer_set_allowed(&self, _timer_id: usize) {}
    fn on_timer_set_denied(&self, _timer_id: usize, error: Error) {
        unreachable!("Timer set denied {:?}", error);
    }

    fn on_timeout(&self, _timer_id: usize) {}
}

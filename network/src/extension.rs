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

use cio::IoError;
use rlp::Encodable;
use time::Duration;

use super::NodeId;
pub use cio::TimerToken;

#[derive(Debug)]
pub enum Error {
    ExtensionDropped,
    DuplicatedTimerId,
    NoMoreTimerToken,
    IoError(IoError),
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Error::IoError(err)
    }
}

pub type Result<T> = result::Result<T, Error>;

pub trait Api: Send + Sync {
    fn send(&self, node: &NodeId, message: &[u8]);

    fn set_timer(&self, timer: TimerToken, d: Duration) -> Result<()>;
    fn set_timer_once(&self, timer: TimerToken, d: Duration) -> Result<()>;
    fn clear_timer(&self, timer: TimerToken) -> Result<()>;

    fn send_local_message(&self, message: &Encodable);
}

pub trait Extension: Send + Sync {
    fn name(&self) -> String;
    fn need_encryption(&self) -> bool;
    fn versions(&self) -> Vec<u64>;

    fn on_initialize(&self, api: Arc<Api>);

    fn on_node_added(&self, _node: &NodeId, _version: u64) {}
    fn on_node_removed(&self, _node: &NodeId) {}

    fn on_message(&self, _node: &NodeId, _message: &[u8]) {}

    fn on_timeout(&self, _timer: TimerToken) {}

    fn on_local_message(&self, _message: &[u8]) {}
}

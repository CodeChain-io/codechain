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

use cio::{IoError, StreamToken};
use rlp::Encodable;

pub use cio::TimerToken;

pub type NodeToken = StreamToken;

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
    fn send(&self, node: &NodeToken, message: &Vec<u8>);
    fn connect(&self, node: &NodeToken);

    fn set_timer(&self, timer: TimerToken, ms: u64);
    fn set_timer_once(&self, timer: TimerToken, ms: u64);
    fn clear_timer(&self, timer: TimerToken);

    fn set_timer_sync(&self, timer: TimerToken, ms: u64) -> Result<()>;
    fn set_timer_once_sync(&self, timer: TimerToken, ms: u64) -> Result<()>;
    fn clear_timer_sync(&self, timer: TimerToken) -> Result<()>;

    fn send_local_message(&self, message: &Encodable);
}

pub trait Extension: Send + Sync {
    fn name(&self) -> String;
    fn need_encryption(&self) -> bool;

    fn on_initialize(&self, api: Arc<Api>);

    fn on_node_added(&self, _node: &NodeToken) {}
    fn on_node_removed(&self, _node: &NodeToken) {}

    fn on_connected(&self, _node: &NodeToken) {}
    fn on_connection_allowed(&self, _node: &NodeToken) {}
    fn on_connection_denied(&self, _node: &NodeToken, _error: Error) {}

    fn on_message(&self, _node: &NodeToken, _message: &Vec<u8>) {}

    fn on_close(&self) {}

    fn on_timer_set_allowed(&self, _timer: TimerToken) {}
    fn on_timer_set_denied(&self, _timer: TimerToken, error: Error) {
        unreachable!("Timer set denied {:?}", error);
    }

    fn on_timeout(&self, _timer: TimerToken) {}

    fn on_local_message(&self, _message: &[u8]) {}
}

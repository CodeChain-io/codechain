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

use crate::NodeId;
use cio::IoError;
use ctimer::{TimerScheduleError, TimerToken};
use primitives::Bytes;
use std::convert::From;
use std::result;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
pub enum Error {
    ExtensionDropped,
    DuplicatedTimerId,
    NoMoreTimerToken,
    IoError(IoError),
    TimerScheduleError(TimerScheduleError),
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Error::IoError(err)
    }
}

impl From<TimerScheduleError> for Error {
    fn from(err: TimerScheduleError) -> Self {
        Error::TimerScheduleError(err)
    }
}

pub type Result<T> = result::Result<T, Error>;

pub trait Api {
    fn send(&self, node: &NodeId, message: Arc<Bytes>);

    fn set_timer(&self, timer: TimerToken, d: Duration) -> Result<()>;
    fn set_timer_once(&self, timer: TimerToken, d: Duration) -> Result<()>;
    fn clear_timer(&self, timer: TimerToken) -> Result<()>;
}

pub trait Extension<Event: Send> {
    fn name() -> &'static str;
    fn need_encryption() -> bool;
    fn versions() -> &'static [u64];

    fn on_node_added(&mut self, _node: &NodeId, _version: u64) {}
    fn on_node_removed(&mut self, _node: &NodeId) {}

    fn on_message(&mut self, _node: &NodeId, _message: &[u8]) {}

    fn on_timeout(&mut self, _token: TimerToken) {}

    fn on_event(&mut self, _event: Event) {}
}

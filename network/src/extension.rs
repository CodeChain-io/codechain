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

use std::convert::From;
use std::result;

use cio::IoError;
use time::Duration;

use crate::NodeId;
use ctimer::{TimeoutHandler, TimerScheduleError, TimerToken};

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

pub trait Api: Send + Sync {
    fn send(&self, node: &NodeId, message: &[u8]);

    fn set_timer(&self, timer: TimerToken, d: Duration) -> Result<()>;
    fn set_timer_once(&self, timer: TimerToken, d: Duration) -> Result<()>;
    fn clear_timer(&self, timer: TimerToken) -> Result<()>;
}

pub trait Extension: TimeoutHandler + Send + Sync {
    fn name(&self) -> &'static str;
    fn need_encryption(&self) -> bool;
    fn versions(&self) -> &[u64];

    fn on_initialize(&self);

    fn on_node_added(&self, _node: &NodeId, _version: u64) {}
    fn on_node_removed(&self, _node: &NodeId) {}

    fn on_message(&self, _node: &NodeId, _message: &[u8]) {}
}

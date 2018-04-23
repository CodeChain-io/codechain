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

use std::fmt;
use std::sync::Arc;

use cio::{IoContext, IoHandler, IoHandlerResult, TimerToken};
use parking_lot::Mutex;
use time::Duration;

use super::super::client::Client;
use super::timer_info::TimerInfo;

type TimerId = usize;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum Message {
    SetTimer {
        extension_name: String,
        timer_id: TimerId,
        duration: Duration,
    },
    SetTimerOnce {
        extension_name: String,
        timer_id: TimerId,
        duration: Duration,
    },
    ClearTimer {
        extension_name: String,
        timer_id: TimerId,
    },
    LocalMessage {
        extension_name: String,
        message: Vec<u8>,
    },
    InitializeExtension {
        extension_name: String,
    },
}

#[derive(Debug)]
enum Error {
    InvalidTimer(TimerToken),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::InvalidTimer(_) => fmt::Debug::fmt(self, f),
        }
    }
}

pub struct Handler {
    client: Arc<Client>,
    timer: Mutex<TimerInfo>,
}

const FIRST_TIMER_TOKEN: TimerToken = 0;
const MAX_TIMERS: usize = 100;
const LAST_TIMER_TOKEN: TimerToken = FIRST_TIMER_TOKEN + MAX_TIMERS;

impl Handler {
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            timer: Mutex::new(TimerInfo::new(FIRST_TIMER_TOKEN, MAX_TIMERS)),
        }
    }
}

impl IoHandler<Message> for Handler {
    fn timeout(&self, _io: &IoContext<Message>, token: TimerToken) -> IoHandlerResult<()> {
        match token {
            FIRST_TIMER_TOKEN...LAST_TIMER_TOKEN => {
                let mut timer = self.timer.lock();
                let info = timer.get_info(token).ok_or(Error::InvalidTimer(token))?;
                if info.once {
                    timer.remove_by_token(token);
                }
                self.client.on_timeout(&info.name, info.timer_id);
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn message(&self, io: &IoContext<Message>, message: &Message) -> IoHandlerResult<()> {
        match *message {
            Message::SetTimer {
                ref extension_name,
                timer_id,
                duration,
            } => {
                let mut timer = self.timer.lock();
                let token = timer.insert(extension_name.clone(), timer_id, false)?;
                io.register_timer(token, duration.num_milliseconds() as u64)?;
                Ok(())
            }
            Message::SetTimerOnce {
                ref extension_name,
                timer_id,
                duration,
            } => {
                let mut timer = self.timer.lock();
                let token = timer.insert(extension_name.clone(), timer_id, true)?;
                io.register_timer_once(token, duration.num_milliseconds() as u64)?;
                Ok(())
            }
            Message::ClearTimer {
                ref extension_name,
                timer_id,
            } => {
                let mut timer = self.timer.lock();
                let token = timer.remove_by_info(extension_name.clone(), timer_id).expect("Unexpected timer id");
                io.clear_timer(token)?;
                Ok(())
            }
            Message::LocalMessage {
                ref extension_name,
                ref message,
            } => {
                self.client.on_local_message(extension_name, message);
                Ok(())
            }
            Message::InitializeExtension {
                ref extension_name,
            } => {
                self.client.initialize_extension(extension_name);
                Ok(())
            }
        }
    }
}

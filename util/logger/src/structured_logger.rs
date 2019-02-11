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

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};

use parking_lot::Mutex;
use serde_derive::Serialize;
use serde_json;
use serde_json::to_value;

const MAX_LOG_SIZE: usize = 400; // MB;

pub struct StructuredLogger {
    // Wrap sender with mutex to get Sync trait
    // To use in global.
    sender: Mutex<Sender<serde_json::Value>>,
    receiver: Mutex<Receiver<serde_json::Value>>,
    enabled: AtomicBool,
    log_size: AtomicUsize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Log {
    pub level: String,
    pub target: String,
    pub message: String,
    pub timestamp: String,
    pub thread_name: String,
}

impl Log {
    fn get_size(&self) -> usize {
        let Log {
            level,
            target,
            message,
            timestamp,
            thread_name,
        } = self;
        level.len() + target.len() + message.len() + timestamp.len() + thread_name.len()
    }
}

impl StructuredLogger {
    pub fn create() -> StructuredLogger {
        let (sender, receiver) = channel();
        StructuredLogger {
            sender: Mutex::new(sender),
            receiver: Mutex::new(receiver),
            enabled: AtomicBool::new(false),
            log_size: AtomicUsize::new(0),
        }
    }

    fn enable(&self) {
        self.enabled.store(true, Ordering::SeqCst);
    }

    fn is_enable(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn log(&self, log: Log) {
        if !self.is_enable() {
            return
        }

        if self.add_and_check_overflow(log.get_size()) {
            // Remove all prev logs
            self.get_logs();
            eprintln!("{} {} {} {}  {}", log.timestamp, log.thread_name, "Error", "LOGGER", "Log overflowed");
        }

        let serialized_log = to_value(log).expect("Log only has String type of fields. It always success");
        let sender = self.sender.lock().clone();
        sender
            .send(serialized_log)
            .expect("StructuredLogger is used as a global variable. Receiver will not dropped before sender.")
    }

    fn add_and_check_overflow(&self, new_log: usize) -> bool {
        // if prev > MAX {
        //   size = new_log
        // } else {
        //   size = prev + new_logx
        // }
        loop {
            let prev_size = self.log_size.load(Ordering::SeqCst);
            if prev_size > MAX_LOG_SIZE * 1024 * 1024 {
                if prev_size == self.log_size.compare_and_swap(prev_size, new_log, Ordering::SeqCst) {
                    return true
                } else {
                    continue
                }
            } else {
                self.log_size.fetch_add(new_log, Ordering::SeqCst);
                return false
            }
        }
    }

    pub fn get_logs(&self) -> Vec<serde_json::Value> {
        self.enable();

        let receiver = self.receiver.lock();
        self.log_size.store(0, Ordering::SeqCst);
        receiver.try_iter().collect()
    }
}

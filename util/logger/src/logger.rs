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

use std::env;
use std::thread;
use time;

use atty;
use colored::Colorize;
use crate::slogger;
use crate::structured_logger;
use env_logger::filter::{Builder as FilterBuilder, Filter};
use log::{LevelFilter, Log, Metadata, Record};

pub struct Config {
    pub instance_id: usize,
}

impl Config {
    pub fn new(instance_id: usize) -> Self {
        Self {
            instance_id,
        }
    }
}

pub struct Logger {
    instance_id: usize,
    filter: Filter,
}

impl Logger {
    pub fn new(config: &Config) -> Self {
        let mut builder = FilterBuilder::new();
        builder.filter(None, LevelFilter::Info);

        if let Ok(rust_log) = env::var("RUST_LOG") {
            builder.parse(&rust_log);
        }

        Self {
            instance_id: config.instance_id,
            filter: builder.build(),
        }
    }

    pub fn filter(&self) -> LevelFilter {
        self.filter.filter()
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if self.filter.matches(record) {
            let thread_name = thread::current().name().unwrap_or_default().to_string();
            let timestamp = time::strftime("%Y-%m-%d %H:%M:%S %Z", &time::now()).unwrap();

            let stderr_isatty = atty::is(atty::Stream::Stderr);
            let instance_id = self.instance_id;
            let timestamp = if stderr_isatty {
                timestamp.bold()
            } else {
                timestamp.normal()
            };
            let thread_name = if stderr_isatty {
                thread_name.blue().bold()
            } else {
                thread_name.normal()
            };
            let log_level = record.level();
            let log_target = record.target();
            let log_message = record.args();
            eprintln!("#{} {} {} {} {}  {}", instance_id, timestamp, thread_name, log_level, log_target, log_message);

            let rfc3339with_nano_second = "%Y-%m-%dT%H:%M:%S.%f%z";
            let timestamp = time::strftime(rfc3339with_nano_second, &time::now()).unwrap();

            slogger.log(structured_logger::Log {
                level: log_level.to_string(),
                target: log_target.to_string(),
                message: log_message.to_string(),
                timestamp,
            });
        }
    }

    fn flush(&self) {}
}

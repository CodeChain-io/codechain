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

use std::time::{SystemTime, UNIX_EPOCH};
use time;

use atty;
use env_logger::filter::{Builder as FilterBuilder, Filter};
use log::{LevelFilter, Log, Metadata, Record};

pub struct Config {
    pub instance_id: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            instance_id: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Current time should be later than unix epoch")
                .subsec_nanos() as usize,
        }
    }
}

pub struct Logger {
    instance_id: usize,
    filter: Filter,
}

impl Logger {
    pub fn new(config: &Config) -> Self {
        let mut builder = FilterBuilder::from_env("RUST_LOG");
        builder.filter(None, LevelFilter::Info);

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
            let timestamp = time::strftime("%Y-%m-%d %H:%M:%S %Z", &time::now()).unwrap();
            eprintln!("#{} {} {} {}  {}", self.instance_id, timestamp, record.level(), record.target(), record.args());
        }
    }

    fn flush(&self) {}
}

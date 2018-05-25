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

extern crate atty;
extern crate colored;
extern crate env_logger;
extern crate log;
extern crate time;

mod logger;
mod macros;

use log::SetLoggerError;

pub use logger::Config as LoggerConfig;
use logger::Logger;

pub use log::Level;

pub fn init(config: &LoggerConfig) -> Result<(), SetLoggerError> {
    let logger = Logger::new(config);
    log::set_max_level(logger.filter());
    log::set_boxed_logger(Box::new(logger))
}

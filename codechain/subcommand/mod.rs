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

mod account_command;
mod convert_command;

use clap::ArgMatches;

use self::account_command::run_account_command;
use self::convert_command::run_convert_command;

pub fn run_subcommand(matches: &ArgMatches) -> Result<(), String> {
    let subcommand = matches.subcommand.as_ref().unwrap();
    match subcommand.name.as_str() {
        "account" => run_account_command(&subcommand.matches),
        "convert" => run_convert_command(&subcommand.matches),
        "commit-hash" => {
            println!("{}", env!("VERGEN_SHA"));
            Ok(())
        }
        _ => Err("Invalid subcommand".to_string()),
    }
}

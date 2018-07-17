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

use std::fs;

use rpassword;

use ccore::AccountProvider;
use ckeystore::accounts_dir::RootDiskDirectory;
use ckeystore::KeyStore;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};

pub fn run_account_command(matches: ArgMatches) -> Result<(), String> {
    if matches.subcommand.is_none() {
        println!("{}", matches.usage());
        return Ok(())
    }

    clogger::init(&LoggerConfig::new(0)).expect("Logger must be successfully initialized");

    let keys_path = matches.value_of("keys-path").unwrap_or("keys");
    let dir = RootDiskDirectory::create(keys_path).expect("Cannot read key path directory");
    let keystore = KeyStore::open(Box::new(dir)).unwrap();
    let ap = AccountProvider::new(keystore);

    match matches.subcommand() {
        ("create", _) => {
            if let Some(password) = read_password_and_confirm() {
                let (address, _) = ap.new_account_and_public(password.as_ref()).expect("Cannot create account");
                println!("Address {} is created", address);
            } else {
                println!("The password does not match");
            }
            Ok(())
        }
        ("import", Some(matches)) => {
            let json_path = matches.value_of("JSON_PATH").expect("JSON_PATH arg is required and its index is 1");
            match fs::read(json_path) {
                Ok(json) => {
                    let password = rpassword::prompt_password_stdout("Password: ").unwrap();
                    match ap.import_wallet(json.as_slice(), password.as_ref()) {
                        Ok(address) => {
                            println!("{:?}", address);
                        }
                        Err(e) => return Err(format!("{}", e)),
                    }
                }
                Err(e) => return Err(format!("{}", e)),
            }
            Ok(())
        }
        ("list", _) => {
            let addresses = ap.get_list().expect("Cannot get account list");
            for address in addresses {
                println!("{:?}", address)
            }
            Ok(())
        }
        _ => Err("Invalid subcommand".to_string()),
    }
}

fn read_password_and_confirm() -> Option<String> {
    let first = rpassword::prompt_password_stdout("Password: ").unwrap();
    let second = rpassword::prompt_password_stdout("Confirm Password: ").unwrap();
    if first == second {
        Some(first)
    } else {
        None
    }
}

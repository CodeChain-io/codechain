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
use std::str::FromStr;

use rpassword;

use ccore::AccountProvider;
use ckey::{FullAddress, Private};
use ckeystore::accounts_dir::RootDiskDirectory;
use ckeystore::KeyStore;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};

use super::constants::DEFAULT_KEYS_PATH;
use super::constants::DEFAULT_NETWORK_ID;

pub fn run_account_command(matches: ArgMatches) -> Result<(), String> {
    if matches.subcommand.is_none() {
        println!("{}", matches.usage());
        return Ok(())
    }

    clogger::init(&LoggerConfig::new(0)).expect("Logger must be successfully initialized");

    let keys_path = matches.value_of("keys-path").unwrap_or(DEFAULT_KEYS_PATH);
    let network_id = DEFAULT_NETWORK_ID;
    let dir = RootDiskDirectory::create(keys_path).expect("Cannot read key path directory");
    let keystore = KeyStore::open(Box::new(dir)).unwrap();
    let ap = AccountProvider::new(keystore);

    match matches.subcommand() {
        ("create", _) => {
            if let Some(password) = read_password_and_confirm() {
                let (address, _) = ap.new_account_and_public(password.as_ref()).expect("Cannot create account");
                println!(
                    "{}",
                    FullAddress::create_version0(network_id, address).expect("The network id is hardcoded to 0x11")
                );
            } else {
                return Err("The password does not match".to_string())
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
                            println!(
                                "{}",
                                FullAddress::create_version0(network_id, address)
                                    .expect("The network id is hardcoded to 0x11")
                            );
                        }
                        Err(e) => return Err(format!("{}", e)),
                    }
                }
                Err(e) => return Err(format!("{}", e)),
            }
            Ok(())
        }
        ("import-raw", Some(matches)) => {
            let key = {
                let val = matches.value_of("RAW_KEY").expect("RAW_KEY arg is required and its index is 1");
                read_raw_key(val)
            };
            match Private::from_str(key) {
                Ok(private) => {
                    if let Some(password) = read_password_and_confirm() {
                        match ap.insert_account(private, password.as_ref()) {
                            Ok(address) => println!(
                                "{}",
                                FullAddress::create_version0(network_id, address)
                                    .expect("The network id is hardcoded to 0x11")
                            ),
                            Err(e) => return Err(format!("{:?}", e)),
                        }
                    } else {
                        return Err("The password does not match".to_string())
                    }
                }
                Err(e) => return Err(format!("{:?}", e)),
            }
            Ok(())
        }
        ("list", _) => {
            let addresses = ap.get_list().expect("Cannot get account list");
            for address in addresses {
                println!(
                    "{}",
                    FullAddress::create_version0(network_id, address).expect("The network id is hardcoded to 0x11")
                )
            }
            Ok(())
        }
        ("remove", Some(matches)) => {
            let key = matches.value_of("ADDRESS").expect("ADDRESS arg is required and its index is 1");
            match FullAddress::from_str(key) {
                Ok(full_address) => {
                    let password = rpassword::prompt_password_stdout("Password: ").unwrap();
                    match ap.remove_account(full_address.address, password.as_ref()) {
                        Ok(_) => println!("{} is deleted", full_address),
                        Err(e) => return Err(format!("{:?}", e)),
                    }
                }
                Err(e) => return Err(format!("{:?}", e)),
            }
            Ok(())
        }
        ("change-password", Some(matches)) => {
            let key = matches.value_of("ADDRESS").expect("ADDRESS arg is required and its index is 1");
            match FullAddress::from_str(key) {
                Ok(full_address) => {
                    let old_password = rpassword::prompt_password_stdout("Old Password: ").unwrap();
                    if let Some(new_password) = read_password_and_confirm() {
                        match ap.change_password(full_address.address, old_password.as_ref(), new_password.as_ref()) {
                            Ok(_) => println!("Password has changed"),
                            Err(e) => return Err(format!("{:?}", e)),
                        }
                    } else {
                        return Err("The password does not match".to_string())
                    }
                }
                Err(e) => return Err(format!("{:?}", e)),
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

fn read_raw_key(val: &str) -> &str {
    if val.starts_with("0x") {
        &val[2..]
    } else {
        &val[..]
    }
}

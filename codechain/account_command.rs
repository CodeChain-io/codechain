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
use ckey::{Password, PlatformAddress, Private};
use ckeystore::accounts_dir::RootDiskDirectory;
use ckeystore::KeyStore;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
use primitives::clean_0x;

use super::constants::DEFAULT_KEYS_PATH;

pub fn run_account_command(matches: ArgMatches) -> Result<(), String> {
    if matches.subcommand.is_none() {
        println!("{}", matches.usage());
        return Ok(())
    }

    clogger::init(&LoggerConfig::new(0)).expect("Logger must be successfully initialized");

    let keys_path = matches.value_of("keys-path").unwrap_or(DEFAULT_KEYS_PATH);
    let dir = RootDiskDirectory::create(keys_path).expect("Cannot read key path directory");
    let keystore = KeyStore::open(Box::new(dir)).unwrap();
    let ap = AccountProvider::new(keystore);

    match matches.subcommand() {
        ("create", _) => create(&ap),
        ("import", Some(matches)) => {
            let json_path = matches.value_of("JSON_PATH").expect("JSON_PATH arg is required and its index is 1");
            import(&ap, json_path)
        }
        ("import-raw", Some(matches)) => {
            let raw_key = matches.value_of("RAW_KEY").expect("RAW_KEY arg is required and its index is 1");
            import_raw(&ap, raw_key)
        }
        ("list", _) => list(&ap),
        ("remove", Some(matches)) => {
            let address = matches.value_of("ADDRESS").expect("ADDRESS arg is required and its index is 1");
            remove(&ap, address)
        }
        ("change-password", Some(matches)) => {
            let address = matches.value_of("ADDRESS").expect("ADDRESS arg is required and its index is 1");
            change_password(&ap, address)
        }
        _ => Err("Invalid subcommand".to_string()),
    }
}

fn create(ap: &AccountProvider) -> Result<(), String> {
    if let Some(password) = read_password_and_confirm() {
        let (address, _) = ap.new_account_and_public(&password).expect("Cannot create account");
        // FIXME: Don't hardcode network_id.
        println!("{}", PlatformAddress::create_version0("cc".into(), address));
    } else {
        return Err("The password does not match".to_string())
    }
    Ok(())
}

fn import(ap: &AccountProvider, json_path: &str) -> Result<(), String> {
    match fs::read(json_path) {
        Ok(json) => {
            let password = prompt_password("Password: ");
            match ap.import_wallet(json.as_slice(), &password) {
                Ok(address) => {
                    // FIXME: Don't hardcode network_id.
                    println!("{}", PlatformAddress::create_version0("cc".into(), address));
                }
                Err(e) => return Err(format!("{}", e)),
            }
        }
        Err(e) => return Err(format!("{}", e)),
    }
    Ok(())
}

fn import_raw(ap: &AccountProvider, raw_key: &str) -> Result<(), String> {
    match Private::from_str(clean_0x(raw_key)) {
        Ok(private) => {
            if let Some(password) = read_password_and_confirm() {
                // FIXME: Don't hardcode network_id.
                match ap.insert_account(private, &password) {
                    Ok(address) => println!("{}", PlatformAddress::create_version0("cc".into(), address)),
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

fn remove(ap: &AccountProvider, address: &str) -> Result<(), String> {
    match PlatformAddress::from_str(address) {
        Ok(full_address) => {
            let password = prompt_password("Password: ");
            match ap.remove_account(full_address.address, &password) {
                Ok(_) => println!("{} is deleted", full_address),
                Err(e) => return Err(format!("{:?}", e)),
            }
        }
        Err(e) => return Err(format!("{:?}", e)),
    }
    Ok(())
}

fn list(ap: &AccountProvider) -> Result<(), String> {
    let addresses = ap.get_list().expect("Cannot get account list");
    for address in addresses {
        // FIXME: Don't hardcode network_id.
        println!("{}", PlatformAddress::create_version0("tc".into(), address))
    }
    Ok(())
}

fn change_password(ap: &AccountProvider, address: &str) -> Result<(), String> {
    match PlatformAddress::from_str(address) {
        Ok(full_address) => {
            let old_password = prompt_password("Old Password: ");
            if let Some(new_password) = read_password_and_confirm() {
                match ap.change_password(full_address.address, &old_password, &new_password) {
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

fn prompt_password(prompt: &str) -> Password {
    Password::from(rpassword::prompt_password_stdout(prompt).unwrap())
}

fn read_password_and_confirm() -> Option<Password> {
    let first = rpassword::prompt_password_stdout("Password: ").unwrap();
    let second = rpassword::prompt_password_stdout("Confirm Password: ").unwrap();
    if first == second {
        Some(Password::from(first))
    } else {
        None
    }
}

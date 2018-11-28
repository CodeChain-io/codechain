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
use ckey::{NetworkId, Password, PlatformAddress, Private};
use ckeystore::accounts_dir::RootDiskDirectory;
use ckeystore::KeyStore;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
use primitives::clean_0x;

use crate::config::ChainType;
use crate::constants::DEFAULT_KEYS_PATH;

pub fn run_account_command(matches: &ArgMatches) -> Result<(), String> {
    if matches.subcommand.is_none() {
        println!("{}", matches.usage());
        return Ok(())
    }

    clogger::init(&LoggerConfig::new(0)).expect("Logger must be successfully initialized");

    let keys_path = get_global_argument(matches, "keys-path").unwrap_or_else(|| DEFAULT_KEYS_PATH.into());
    let dir = RootDiskDirectory::create(keys_path).expect("Cannot read key path directory");
    let keystore = KeyStore::open(Box::new(dir)).unwrap();
    let ap = AccountProvider::new(keystore);
    let chain = get_global_argument(matches, "chain").unwrap_or_else(|| "solo".into());
    let chain_type: ChainType = chain.parse().unwrap();
    let network_id: NetworkId = chain_type.scheme().map(|scheme| scheme.params().network_id)?;

    match matches.subcommand() {
        ("create", _) => create(&ap, network_id),
        ("import", Some(matches)) => {
            let json_path = matches.value_of("JSON_PATH").expect("JSON_PATH arg is required and its index is 1");
            import(&ap, network_id, json_path)
        }
        ("import-raw", Some(matches)) => {
            let raw_key = matches.value_of("RAW_KEY").expect("RAW_KEY arg is required and its index is 1");
            import_raw(&ap, network_id, raw_key)
        }
        ("list", _) => list(&ap, network_id),
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

fn create(ap: &AccountProvider, network_id: NetworkId) -> Result<(), String> {
    let password = read_password_and_confirm().ok_or("The password does not match")?;
    let (address, _) = ap.new_account_and_public(&password).expect("Cannot create account");
    println!("{}", PlatformAddress::new_v1(network_id, address));
    Ok(())
}

fn import(ap: &AccountProvider, network_id: NetworkId, json_path: &str) -> Result<(), String> {
    let json = fs::read(json_path).map_err(|err| err.to_string())?;
    let password = prompt_password("Password: ");
    let address = ap.import_wallet(json.as_slice(), &password).map_err(|err| err.to_string())?;
    println!("{}", PlatformAddress::new_v1(network_id, address));
    Ok(())
}

fn import_raw(ap: &AccountProvider, network_id: NetworkId, raw_key: &str) -> Result<(), String> {
    let private = Private::from_str(clean_0x(raw_key)).map_err(|err| err.to_string())?;
    let password = read_password_and_confirm().ok_or("The password does not match")?;
    let address = ap.insert_account(private, &password).map_err(|err| err.to_string())?;
    println!("{}", PlatformAddress::new_v1(network_id, address));
    Ok(())
}

fn remove(ap: &AccountProvider, address: &str) -> Result<(), String> {
    let address = PlatformAddress::from_str(address).map_err(|err| err.to_string())?;
    if confirmation_dialog("REMOVE")? {
        ap.remove_account(address.into_address()).map_err(|err| err.to_string())?;
        println!("{} is deleted", address);
        Ok(())
    } else {
        Err(format!("Confirmation failed, {} is not deleted", address))
    }
}

fn list(ap: &AccountProvider, network_id: NetworkId) -> Result<(), String> {
    let addresses = ap.get_list().expect("Cannot get account list");
    for address in addresses {
        println!("{}", PlatformAddress::new_v1(network_id, address))
    }
    Ok(())
}

fn change_password(ap: &AccountProvider, address: &str) -> Result<(), String> {
    let address = PlatformAddress::from_str(address).map_err(|err| err.to_string())?;
    let old_password = prompt_password("Old Password: ");
    let new_password = read_password_and_confirm().ok_or("The password does not match")?;
    ap.change_password(address.into_address(), &old_password, &new_password).map_err(|err| err.to_string())?;
    println!("Password has changed");
    Ok(())
}

fn prompt_password(prompt: &str) -> Password {
    rpassword::prompt_password_stdout(prompt).map(Password::from).unwrap()
}

fn confirmation_dialog(confirm_message: &str) -> Result<bool, String> {
    println!("Type \"{}\" to confirm: ", confirm_message);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).map_err(|e| format!("Failed to read line: {}", e))?;
    Ok(&input[..input.len() - 1] == confirm_message)
}

fn read_password_and_confirm() -> Option<Password> {
    let first = rpassword::prompt_password_stdout("Password: ").unwrap();
    let second = rpassword::prompt_password_stdout("Confirm Password: ").unwrap();
    if first == second {
        Some(first.into())
    } else {
        None
    }
}

fn get_global_argument(matches: &ArgMatches, arg_name: &str) -> Option<String> {
    match matches.value_of(arg_name) {
        Some(value) => Some(value.to_string()),
        None => match matches.subcommand() {
            (_, Some(matches)) => matches.value_of(arg_name).map(ToString::to_string),
            _ => None,
        },
    }
}

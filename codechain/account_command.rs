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

use ccore::AccountProvider;
use ckeys::KeyPair;
use ckeystore::accounts_dir::RootDiskDirectory;
use ckeystore::KeyStore;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};

pub fn run_account_command(matches: ArgMatches) -> Result<(), String> {
    clogger::init(&LoggerConfig::new(0)).expect("Logger must be successfully initialized");

    let subcommand = matches.subcommand.unwrap();
    // FIXME : Add cli option.
    let dir = RootDiskDirectory::create("keystoreData").expect("Cannot read key path directory");
    let keystore = KeyStore::open(Box::new(dir)).unwrap();
    let ap = AccountProvider::new(keystore);

    match subcommand.name.as_ref() {
        "create" => {
            let (address, _) = ap.new_account_and_public().expect("Cannot create account");
            info!("Addresss {} is created", address);
            Ok(())
        }
        "import" => {
            let keystring = subcommand.matches.value_of("raw-key").unwrap();
            let keypair = KeyPair::from_private(keystring.parse().unwrap()).unwrap();
            ap.insert_account(keypair.private().clone()).expect("Cannot insert account");
            Ok(())
        }
        "list" => {
            let addresses = ap.get_list().expect("Cannot get account list");
            for address in addresses {
                println!("{:?}", address)
            }
            Ok(())
        }
        _ => Err("Invalid subcommand".to_string()),
    }
}

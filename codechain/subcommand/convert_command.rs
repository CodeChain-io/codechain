// Copyright 2019 Kodebox, Inc.
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

use std::io::stdin;
use std::ops::Deref;
use std::str::FromStr;

use ckey::hex::ToHex;
use ckey::{public_to_address, Address, KeyPair, NetworkId, PlatformAddress, Private, Public};
use clap::ArgMatches;

use crate::config::ChainType;
use primitives::remove_0x_prefix;

pub fn run_convert_command(matches: &ArgMatches) -> Result<(), String> {
    let from = matches.value_of("from").expect("Argument 'from' is required");
    let to = matches.value_of("to").expect("Argument 'to' is required");

    let mut input = String::new();
    stdin().read_line(&mut input).map_err(|e| e.to_string())?;
    let result = convert(from, to, &input.trim(), || get_network_id(matches))?;
    println!("{}", result);
    Ok(())
}

fn convert(
    from: &str,
    to: &str,
    input: &str,
    get_network_id: impl FnOnce() -> Result<NetworkId, String>,
) -> Result<String, String> {
    match (from, to) {
        ("private", "private") => {
            let private = get_private(input)?;
            Ok(private.to_hex())
        }
        ("private", "public") => {
            let private = get_private(input)?;
            let public = private_to_public(private)?;
            Ok(public.to_hex())
        }
        ("private", "address") => {
            let private = get_private(input)?;
            let public = private_to_public(private)?;
            let address = public_to_address(&public);
            Ok(format!("{:x}", address.deref()))
        }
        ("private", "accountId") => {
            let network_id = get_network_id()?;

            let private = get_private(input)?;
            let public = private_to_public(private)?;
            let address = public_to_address(&public);
            let account_id = PlatformAddress::new_v1(network_id, address);
            Ok(account_id.to_string())
        }
        ("public", "public") => {
            let public = get_public(input)?;
            Ok(public.to_hex())
        }
        ("public", "address") => {
            let public = get_public(input)?;
            let address = public_to_address(&public);
            Ok(format!("{:x}", address.deref()))
        }
        ("public", "accountId") => {
            let network_id = get_network_id()?;

            let public = get_public(input)?;
            let address = public_to_address(&public);
            let account_id = PlatformAddress::new_v1(network_id, address);
            Ok(account_id.to_string())
        }
        ("address", "address") => {
            let address = get_address(input)?;
            Ok(format!("{:x}", address.deref()))
        }
        ("address", "accountId") => {
            let network_id = get_network_id()?;

            let address = get_address(input)?;
            let account_id = PlatformAddress::new_v1(network_id, address);
            Ok(account_id.to_string())
        }
        ("accountId", "accountId") => {
            let account_id = get_account_id(input)?;
            Ok(account_id.to_string())
        }
        ("accountId", "address") => {
            let account_id = get_account_id(input)?;
            let address = account_id.into_address();
            Ok(format!("{:x}", address.deref()))
        }
        (..) => Err(format!("Cannot convert from {} to {}", from, to)),
    }
}

fn get_public(input: &str) -> Result<Public, String> {
    Public::from_str(remove_0x_prefix(input)).map_err(|e| format!("Error on reading public key: {}", e))
}

fn get_private(input: &str) -> Result<Private, String> {
    Private::from_str(remove_0x_prefix(input)).map_err(|e| format!("Error on reading private key: {}", e))
}

fn get_address(input: &str) -> Result<Address, String> {
    Address::from_str(input).map_err(|e| format!("Error on reading address: {}", e))
}

fn get_account_id(input: &str) -> Result<PlatformAddress, String> {
    PlatformAddress::from_str(input).map_err(|e| format!("Error on reading accountId: {}", e))
}

fn private_to_public(private: Private) -> Result<Public, String> {
    let keypair =
        KeyPair::from_private(private).map_err(|e| format!("Error on converting private key to public key: {}", e))?;
    Ok(*keypair.public())
}

fn get_network_id(matches: &ArgMatches) -> Result<NetworkId, String> {
    let chain = matches.value_of("chain").unwrap_or_else(|| "solo");
    let chain_type: ChainType = chain.parse().unwrap();
    // XXX: What should we do if the network id has been changed
    let network_id: NetworkId = chain_type.scheme().map(|scheme| scheme.genesis_params().network_id())?;
    Ok(network_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIVATE_KEY: &str = "fcab1293c1510ef5646763b27f1f70b828e255e1f462dad12fc978a31ba72fbc";
    const PUBLIC_KEY: &str = "0339c7db9ad207a418f1790cb286b17ccf7f5c5ac0d6bf00942c7d010d731c798d7d97ac3cbc1e59e0a10dcae27e0e2f182fa2371589a28c1d49e7161f9ce4bf";
    const ADDRESS: &str = "cfa56ed5085af735a45bcec6d2474c45597a2ab1";
    const ACCOUNT_ID: &str = "tccq8862mk4ppd0wddyt08vd5j8f3z4j732kyhdnfj9";

    fn get_test_network_id() -> Result<NetworkId, String> {
        Ok(NetworkId::from("tc"))
    }

    #[test]
    fn test_private_to_private() {
        let result = convert("private", "private", PRIVATE_KEY, get_test_network_id);
        assert_eq!(result, Ok(PRIVATE_KEY.to_string()));

        let prefixed = format!("0x{}", PRIVATE_KEY);
        let result = convert("private", "private", &prefixed, get_test_network_id);
        assert_eq!(result, Ok(PRIVATE_KEY.to_string()));
    }

    #[test]
    fn test_private_to_public() {
        let result = convert("private", "public", PRIVATE_KEY, get_test_network_id);
        assert_eq!(result, Ok(PUBLIC_KEY.to_string()));

        let prefixed = format!("0x{}", PRIVATE_KEY);
        let result = convert("private", "public", &prefixed, get_test_network_id);
        assert_eq!(result, Ok(PUBLIC_KEY.to_string()));
    }

    #[test]
    fn test_private_to_address() {
        let result = convert("private", "address", PRIVATE_KEY, get_test_network_id);
        assert_eq!(result, Ok(ADDRESS.to_string()));

        let prefixed = format!("0x{}", PRIVATE_KEY);
        let result = convert("private", "address", &prefixed, get_test_network_id);
        assert_eq!(result, Ok(ADDRESS.to_string()));
    }

    #[test]
    fn test_private_to_account_id() {
        let result = convert("private", "accountId", PRIVATE_KEY, get_test_network_id);
        assert_eq!(result, Ok(ACCOUNT_ID.to_string()));

        let prefixed = format!("0x{}", PRIVATE_KEY);
        let result = convert("private", "accountId", &prefixed, get_test_network_id);
        assert_eq!(result, Ok(ACCOUNT_ID.to_string()));
    }

    #[test]
    fn test_public_to_public() {
        let result = convert("public", "public", PUBLIC_KEY, get_test_network_id);
        assert_eq!(result, Ok(PUBLIC_KEY.to_string()));

        let prefixed = format!("0x{}", PUBLIC_KEY);
        let result = convert("public", "public", &prefixed, get_test_network_id);
        assert_eq!(result, Ok(PUBLIC_KEY.to_string()));
    }

    #[test]
    fn test_public_to_address() {
        let result = convert("public", "address", PUBLIC_KEY, get_test_network_id);
        assert_eq!(result, Ok(ADDRESS.to_string()));

        let prefixed = format!("0x{}", PUBLIC_KEY);
        let result = convert("public", "address", &prefixed, get_test_network_id);
        assert_eq!(result, Ok(ADDRESS.to_string()));
    }

    #[test]
    fn test_public_to_account_id() {
        let result = convert("public", "accountId", PUBLIC_KEY, get_test_network_id);
        assert_eq!(result, Ok(ACCOUNT_ID.to_string()));

        let prefixed = format!("0x{}", PUBLIC_KEY);
        let result = convert("public", "accountId", &prefixed, get_test_network_id);
        assert_eq!(result, Ok(ACCOUNT_ID.to_string()));
    }

    #[test]
    fn test_address_to_address() {
        let result = convert("address", "address", ADDRESS, get_test_network_id);
        assert_eq!(result, Ok(ADDRESS.to_string()));

        let prefixed = format!("0x{}", ADDRESS);
        let result = convert("address", "address", &prefixed, get_test_network_id);
        assert_eq!(result, Ok(ADDRESS.to_string()));
    }

    #[test]
    fn test_address_to_account_id() {
        let result = convert("address", "accountId", ADDRESS, get_test_network_id);
        assert_eq!(result, Ok(ACCOUNT_ID.to_string()));

        let prefixed = format!("0x{}", ADDRESS);
        let result = convert("address", "accountId", &prefixed, get_test_network_id);
        assert_eq!(result, Ok(ACCOUNT_ID.to_string()));
    }

    #[test]
    fn test_account_id_to_account_id() {
        let result = convert("accountId", "accountId", ACCOUNT_ID, get_test_network_id);
        assert_eq!(result, Ok(ACCOUNT_ID.to_string()));
    }

    #[test]
    fn test_account_id_to_address() {
        let result = convert("accountId", "address", ACCOUNT_ID, get_test_network_id);
        assert_eq!(result, Ok(ADDRESS.to_string()));
    }
}

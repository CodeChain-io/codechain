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

use std::str::FromStr;

use clap;
use cnetwork::Address;
use rpc::HttpConfiguration as RpcHttpConfig;

pub struct Config {
    pub quiet: bool,
}

pub struct NetworkConfig {
    pub port: u16,
    pub bootstrap_addresses: Vec<Address>,
}

pub fn parse(matches: &clap::ArgMatches) -> Result<Config, String> {
    let quiet = matches.is_present("quiet");

    Ok(Config {
        quiet,
    })
}

pub fn parse_network_config(matches: &clap::ArgMatches) -> Result<Option<NetworkConfig>, String> {
    if matches.is_present("no-network") {
        return Ok(None)
    }

    let bootstrap_addresses = {
        if let Some(addresses) = matches.values_of("bootstrap-addresses") {
            addresses.map(|s| Address::from_str(s).unwrap()).collect::<Vec<_>>()
        } else {
            vec![]
        }
    };

    let port = match matches.value_of("port") {
        Some(port) => port.parse().map_err(|_| "Invalid port".to_owned())?,
        None => 3485,
    };
    Ok(Some(NetworkConfig {
        port,
        bootstrap_addresses,
    }))
}

pub fn parse_rpc_config(matches: &clap::ArgMatches) -> Result<Option<RpcHttpConfig>, String> {
    if matches.is_present("no-jsonrpc") {
        return Ok(None)
    }

    let mut config = RpcHttpConfig::with_port(8080);

    if let Some(port) = matches.value_of("jsonrpc-port") {
        config.port = port.parse().map_err(|_| "Invalid JSON RPC port".to_owned())?;
    }
    if let Some(interface) = matches.value_of("jsonrpc-interface") {
        config.interface = interface.to_owned();
    }
    if let Some(cors) = matches.value_of("jsonrpc-cors") {
        config.cors = Some(vec![cors.parse().map_err(|_| "Invalid JSON RPC CORS".to_owned())?]);
    }
    if let Some(hosts) = matches.value_of("jsonrpc-hosts") {
        config.hosts = Some(vec![hosts.parse().map_err(|_| "Invalid JSON RPC hosts".to_owned())?]);
    }

    Ok(Some(config))
}

use clap;
use rpc::HttpConfiguration as RpcHttpConfig;

pub struct Config {
    pub port: u16,
    pub quiet: bool,
}

pub fn parse(matches: &clap::ArgMatches) -> Result<Config, String> {
    let quiet = matches.is_present("quiet");
    let port = match matches.value_of("port") {
        Some(port) => port.parse().map_err(|_| "Invalid port".to_owned())?,
        None => 3485,
    };

    let config = Config {
        quiet,
        port,
    };
    Ok(config)
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

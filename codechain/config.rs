use clap;

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

	let config = Config { quiet: quiet, port: port };
	Ok(config)
}

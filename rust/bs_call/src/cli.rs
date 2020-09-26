use std::str::FromStr;
use std::io;

use clap::App;

mod cli_utils;
mod options;

use cli_utils::LogLevel;
use super::config::BsCallConfig;

pub fn process_cli() -> io::Result<BsCallConfig> {
	let yaml = load_yaml!("cli/cli.yml");
    let app = App::from_yaml(yaml);

	// Setup logging
	let m = app.get_matches();	
	    let ts = m.value_of("timestamp").map(|v| {
        stderrlog::Timestamp::from_str(v).unwrap_or_else(|_| {
            clap::Error {
                message: "invalid value for 'timestamp'".into(),
                kind: clap::ErrorKind::InvalidValue,
                info: None,
            }.exit()
        })
    }).unwrap_or(stderrlog::Timestamp::Off);
	let verbose = value_t!(m.value_of("loglevel"), LogLevel).unwrap_or_else(|_| LogLevel::from_str("info").expect("Could not set loglevel info"));
	let quiet = verbose.is_none() || m.is_present("quiet");
    stderrlog::new()
        .quiet(quiet)
        .verbosity(verbose.get_level())
        .timestamp(ts)
        .init()
        .unwrap();

	// Process arguments
	options::handle_options(&m)
}
use clap::{App, AppSettings};
use std::str::FromStr;
use crate::commands;

pub mod utils;
use utils::LogLevel;

pub fn process_cli() -> Result<(), &'static str> {
	let yaml = load_yaml!("cli.yml");
    let m = App::from_yaml(yaml)
        .setting(AppSettings::VersionlessSubcommands)
		.get_matches();
		
	// Interpret global command line flags and set up logging
    
    let ts = m.value_of("timestamp").map(|v| {
        stderrlog::Timestamp::from_str(v).unwrap_or_else(|_| {
            clap::Error {
                message: "invalid value for 'timestamp'".into(),
                kind: clap::ErrorKind::InvalidValue,
                info: None,
            }.exit()
        })
    }).unwrap_or(stderrlog::Timestamp::Off);
	let verbose = value_t!(m.value_of("loglevel"), LogLevel).unwrap_or_else(|_| LogLevel::new(0));
	let quiet = verbose.is_none() || m.is_present("quiet");
    stderrlog::new()
        .quiet(quiet)
        .verbosity(verbose.get_level())
        .timestamp(ts)
        .init()
        .unwrap();

	// Now handle subcommands
	
	match m.subcommand() {
		("prepare", Some(m_sum)) => {
			debug!("User entered 'prepare' command");
			commands::prepare::prepare_command(m_sum)
		},
		("index", Some(m_sum)) => {
			debug!("User entered 'index' command");
			commands::index::index_command(m_sum)
		},
		_ => {
			error!("Unknown subcomamnd");
			Err("Unknown subcommand")
		},
	}
}

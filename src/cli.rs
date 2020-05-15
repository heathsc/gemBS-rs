use clap::{App, AppSettings};
use std::str::FromStr;

pub fn process_cli() {
	let yaml = load_yaml!("cli.yml");
    let m = App::from_yaml(yaml)
        .setting(AppSettings::VersionlessSubcommands)
		.get_matches();
    let mut quiet = m.is_present("quiet");
    let ts = m.value_of("timestamp").map(|v| {
        stderrlog::Timestamp::from_str(v).unwrap_or_else(|_| {
            clap::Error {
                message: "invalid value for 'timestamp'".into(),
                kind: clap::ErrorKind::InvalidValue,
                info: None,
            }.exit()
        })
    }).unwrap_or(stderrlog::Timestamp::Off);
	let verbose: usize = m.value_of("loglevel").map(|v| {
		match v {
			"none" => {
				quiet = true;
				0
			},
			"error" => 0,
			"warn" => 1,
			"info" => 2,
			"debug" => 3,
			"trace" => 4,
			_ => 0, // Shouldn't happen
		}
	}).unwrap_or(0);
    stderrlog::new()
        .quiet(quiet)
        .verbosity(verbose)
        .timestamp(ts)
        .init()
        .unwrap();
}

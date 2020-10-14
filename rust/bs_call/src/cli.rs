use std::str::FromStr;
use std::io;

use clap::App;

mod cli_utils;
mod options;

use cli_utils::LogLevel;
use super::process;
use crate::config::BsCallConfig;
use crate::stats;

pub fn process_cli() -> io::Result<BsCallConfig> {
	let yaml = load_yaml!("cli/cli.yml");
    let app = App::from_yaml(yaml);
	let mut vbuf: Vec<u8> = Vec::new();
	app.write_version(&mut vbuf).expect("Error getting version from clap");
	let version = std::str::from_utf8(&vbuf).expect("Version string not utf8");
	
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
	let mut bs_cfg = options::handle_options(&m)?;

	let chash = &bs_cfg.conf_hash;
	let source = format!("{},under_conversion={},over_conversion={},mapq_thresh={},bq_thresh={}", version,
			chash.get_float("under_conversion"), chash.get_float("over_conversion"),
			chash.get_int("mapq_threshold"), chash.get_int("bq_threshold"));

	// Write Output header
	process::write_vcf_header(&mut bs_cfg, &source)?;
	
	// Initialize Stats
	if let Some(s) = bs_cfg.get_conf_str("report_file") {
		bs_cfg.stats = Some(stats::Stats::new(s, &source))
	}
	Ok(bs_cfg)	
}
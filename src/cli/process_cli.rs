use std::str::FromStr;
use std::env;
use std::path::Path;
use clap::{App, AppSettings};

use crate::commands;
use crate::config::GemBS;
use crate::common::defs::{Section, DataValue};

use super::utils::LogLevel;

pub fn process_cli(gem_bs: &mut GemBS) -> Result<(), String> {
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

	if let Some(f) = m.value_of("dir") {
		let wd = Path::new(f);
		env::set_current_dir(&wd).map_err(|e| format!("Can not switch working directory to {}: {}", f, e))?;
		debug!("Moved working directory to {}", f);
	}	

	if let Some(s) = m.value_of("json") { gem_bs.set_config(Section::Default, "json_file", DataValue::String(s.to_string())); }
	if let Some(s) = m.value_of("gembs_root") { gem_bs.set_config(Section::Default, "gembs_root", DataValue::String(s.to_string())); }
	if m.is_present("keep_logs") { gem_bs.set_config(Section::Default, "keep_logs", DataValue::Bool(true)); }
	if m.is_present("ignore_times") { gem_bs.set_ignore_times(true); }
	if m.is_present("ignore_status") { gem_bs.set_ignore_status(true); }
	if m.is_present("all") { gem_bs.set_all(true); }

	info!("Total memory detected: {}", gem_bs.total_mem());
	
	// Now handle subcommands
	
	match m.subcommand() {
		("prepare", Some(m_sum)) => {
			debug!("User entered 'prepare' command");
			commands::prepare::prepare_command(m_sum, gem_bs)
		},
		("index", Some(m_sum)) => {
			debug!("User entered 'index' command");
			commands::index::index_command(m_sum, gem_bs)
		},
		("map", Some(m_sum)) => {
			debug!("User entered 'map' command");
			commands::map::map_command(m_sum, gem_bs)
		},
		("merge-bams", Some(m_sum)) => {
			debug!("User entered 'merge-bams' command");
			commands::map::merge_bams_command(m_sum, gem_bs)
		},
		("call", Some(m_sum)) => {
			debug!("User entered 'call' command");
			commands::call::call_command(m_sum, gem_bs)
		},
		("merge-bcfs", Some(m_sum)) => {
			debug!("User entered 'merge-bcf' command");
			commands::call::merge_bcfs_command(m_sum, gem_bs)
		},
		("index-bcf", Some(m_sum)) => {
			debug!("User entered 'index-bcf' command");
			commands::call::index_bcf_command(m_sum, gem_bs)
		},
		("extract", Some(m_sum)) => {
			debug!("User entered 'extract' command");
			commands::extract::extract_command(m_sum, gem_bs)
		},
		("md5sum", Some(m_sum)) => {
			debug!("User entered 'md5sum' command");
			commands::md5sum::md5sum_command(m_sum, gem_bs)
		},
		("run", Some(m_sum)) => {
			debug!("User entered 'run' command");
			commands::run::run_command(m_sum, gem_bs)
		},
		_ => {
			Err("Unknown subcommand".to_string())
		},
	}
}

use std::str::FromStr;
use std::env;
use std::path::Path;
use clap::{App, AppSettings, ArgGroup};

#[cfg(feature = "slurm")]
use clap::Arg;

use crate::commands;
use crate::config::GemBS;
use crate::common::defs::{Section, DataValue};

use super::utils::LogLevel;

pub fn process_cli(gem_bs: &mut GemBS) -> Result<(), String> {
	let yaml = load_yaml!("cli.yml");

    let mut app = App::from_yaml(yaml);
	app = app.setting(AppSettings::VersionlessSubcommands);
	#[cfg(feature = "slurm")]
	{
		let container: Option<&'static str> = option_env!("GEMBS_CONTAINER");
		app = app.arg(Arg::with_name("slurm_script").short("s").long("slurm-script").takes_value(true)
			.value_name("SCRIPT_FILE").help("Generate PERL script to submit commands to slurm for execution"));
		if container.is_none() {
			app = app.arg(Arg::with_name("slurm").short("S").long("slurm").help("Submit commands to slurm for execution"))
			.group(ArgGroup::with_name("slurm_opts").args(&["slurm", "slurm_script"]));
		}
	}
	let m = app.get_matches();		
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
	let verbose = value_t!(m.value_of("loglevel"), LogLevel).unwrap_or_else(|_| LogLevel::from_str("info").expect("Could not set loglevel info"));
	let quiet = verbose.is_none() || m.is_present("quiet");
    stderrlog::new()
        .quiet(quiet)
        .verbosity(verbose.get_level())
        .timestamp(ts)
        .init()
        .unwrap();
	gem_bs.set_verbose(verbose);
	if let Some(f) = m.value_of("dir") {
		let wd = Path::new(f);
		env::set_current_dir(&wd).map_err(|e| format!("Can not switch working directory to {}: {}", f, e))?;
		debug!("Moved working directory to {}", f);
	}	

	if let Some(s) = m.value_of("json_file") { gem_bs.set_config(Section::Default, "json_file", DataValue::String(s.to_string())); }
	if let Some(s) = m.value_of("gembs_root") { gem_bs.set_config(Section::Default, "gembs_root", DataValue::String(s.to_string())); }
	if m.is_present("keep_logs") { gem_bs.set_config(Section::Default, "keep_logs", DataValue::Bool(true)); }
	if m.is_present("ignore_times") { gem_bs.set_ignore_times(true); }
	if m.is_present("ignore_status") { gem_bs.set_ignore_status(true); }
	if m.is_present("all") { gem_bs.set_all(true); }
	if m.is_present("dry_run") { gem_bs.set_dry_run(true); }
	if m.is_present("slurm") { gem_bs.set_slurm(true); }
	if let Some(s) = m.value_of("json") { gem_bs.set_json_out(s); }
	if let Some(s) = m.value_of("slurm_script") { gem_bs.set_slurm_script(s); }

	let mem = (gem_bs.total_mem() as f64) / 1073741824.0;
	info!("Total memory detected: {:.1} GB", mem);
	
	// Now handle subcommands
	
	match m.subcommand() {
		("prepare", Some(m_sum)) => {
			commands::prepare::prepare_command(m_sum, gem_bs)
		},
		("index", Some(m_sum)) => {
			commands::index::index_command(m_sum, gem_bs)
		},
		("map", Some(m_sum)) => {
			commands::map::map_command(m_sum, gem_bs)
		},
		("call", Some(m_sum)) => {
			commands::call::call_command(m_sum, gem_bs)
		},
		("extract", Some(m_sum)) => {
			commands::extract::extract_command(m_sum, gem_bs)
		},
		("report", Some(m_sum)) => {
			commands::report::report_command(m_sum, gem_bs)
		},
		("run", Some(m_sum)) => {
			commands::run::run_command(m_sum, gem_bs)
		},
		("clear", Some(m_sum)) => {
			commands::clear::clear_command(m_sum, gem_bs)
		},
		_ => {
			Err("Unknown subcommand".to_string())
		},
	}
}

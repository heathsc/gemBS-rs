use std::io;

use clap::App;

mod cli_utils;
mod options;

use utils::log_level::init_log;
use super::process;
use crate::config::{BsCallConfig, BsCallFiles, ConfVar};

pub fn process_cli() -> io::Result<(BsCallConfig, BsCallFiles)> {
	let yaml = load_yaml!("cli/cli.yml");
    let app = App::from_yaml(yaml).version(crate_version!());
	let mut vbuf: Vec<u8> = Vec::new();
	app.write_version(&mut vbuf).expect("Error getting version from clap");
	let version = std::str::from_utf8(&vbuf).expect("Version string not utf8");
	
	// Setup logging
	let m = app.get_matches();	
	let _ = init_log(&m);
	// Process arguments
	let (mut bs_cfg, mut bs_files) = options::handle_options(&m)?;

	let chash = &bs_cfg.conf_hash;
	let source = format!("{},under_conversion={},over_conversion={},mapq_thresh={},bq_thresh={}", version,
			chash.get_float("under_conversion"), chash.get_float("over_conversion"),
			chash.get_int("mapq_threshold"), chash.get_int("bq_threshold"));

	// Write Output header
	process::write_vcf_header(&mut bs_cfg, &mut bs_files, &source)?;
	bs_cfg.conf_hash.set(&"bs_call_source", ConfVar::String(Some(source)));
	Ok((bs_cfg, bs_files))	
}
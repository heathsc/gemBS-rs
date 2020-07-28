use clap::ArgMatches;
use crate::config::GemBS;
use crate::common::defs::{Section, DataValue};
use crate::common::utils;
mod config_file;
pub mod metadata;

pub fn prepare_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	
	gem_bs.setup_fs(true)?;
	
	// Process configuration file	
	// We can just unwrap here because we should only get here if the config option is present,
	// so if it is not present then there has been an internal error an we can panic...
	config_file::process_config_file(m.value_of("config").unwrap(), gem_bs)?;

	if m.is_present("populate") { gem_bs.set_config(Section::Index, "populate_cache", DataValue::Bool(true)); }
	
	// Process sample metadata file
	// This can either be a cvs file or a json file
	if let Some(f) = m.value_of("cvs_metadata") { 
		metadata::process_csv::process_cvs_metatdata_file(f, gem_bs)?;
	} else if let Some(f) = m.value_of("json_metadata") {
		metadata::process_json::process_json_metadata_file(f, gem_bs)?;
	}
	let task_path = gem_bs.get_task_file_path();
	let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?;
	gem_bs.setup_assets_and_tasks(&flock)?;
	
	// Dump JSON config file to disk
	gem_bs.write_json_config()
}

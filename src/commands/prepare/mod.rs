use clap::ArgMatches;
use crate::config::GemBS;
use crate::config::check_ref;
use crate::common::defs::{Section, DataValue};

mod config_file;
pub mod metadata;

pub fn prepare_command(m: &ArgMatches, gem_bs: &mut GemBS, json_option: Option<&str>, root_option: Option<&str>) -> Result<(), String> {
	
	// Process configuration file	
	// We can just unwrap here because we should only get here if the config option is present,
	// so if it is not present then there has been an internal error an we can panic...
	config_file::process_config_file(m.value_of("config").unwrap(), gem_bs)?;

	// Command line arguments overwrite options set in the config file
	if let Some(s) = json_option { gem_bs.set_config(Section::Default, "json_file", DataValue::String(s.to_string())); }
	if let Some(s) = root_option { gem_bs.set_config(Section::Default, "gembs_root", DataValue::String(s.to_string())); }
	if m.is_present("no_db") { gem_bs.set_config(Section::Default, "no_db", DataValue::Bool(true)); }
	if m.is_present("populate") { gem_bs.set_config(Section::Index, "populate_cache", DataValue::Bool(true)); }
	
	gem_bs.setup_fs(true)?;
	
	// Process sample metadata file
	// This can either be a cvs file or a json file
	if let Some(f) = m.value_of("cvs_metadata") { 
		metadata::process_csv::process_cvs_metatdata_file(f, gem_bs)?;
	} else if let Some(f) = m.value_of("json_metadata") {
		metadata::process_json::process_json_metadata_file(f, gem_bs)?;
	}
	
	check_ref::check_ref_and_indices(gem_bs)?;
	
	// Dump JSON config file to disk
	gem_bs.write_json_config()?;
	
	
	Ok(())
}

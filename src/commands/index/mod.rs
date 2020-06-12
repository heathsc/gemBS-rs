use clap::ArgMatches;
use crate::cli::utils;
use crate::config::GemBS;
use crate::common::defs::{Section, DataValue, Command};

pub fn index_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	// Get JSON config file from disk
	gem_bs.read_json_config()?;
	
	if let Some(x) = utils::from_arg_matches(m, "threads") { gem_bs.set_config(Section::Index, "threads", DataValue::Int(x)); }
	if let Some(x) = utils::from_arg_matches(m, "sampling") { gem_bs.set_config(Section::Index, "sampling_rate", DataValue::Int(x)); }

	gem_bs.setup_assets_and_tasks()?;
	for task in gem_bs.get_tasks_iter().filter(|t| t.command() == Command::Extract) {
		println!("{:?} {:?}", task, gem_bs.task_status(task));
	}
	Ok(())
}

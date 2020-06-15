use clap::ArgMatches;
use std::collections::HashSet;
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::common::defs::{Section, Command};

pub fn index_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	// Get JSON config file from disk
	gem_bs.read_json_config()?;
	
	let options = handle_options(m, gem_bs, Section::Index);
	gem_bs.setup_assets_and_tasks()?;
	let mut task_set = HashSet::new();
	if options.contains_key("make_bs_index") { task_set.insert("index"); }
	if options.contains_key("make_nonbs_index") { task_set.insert("nonbs_index"); }
	if options.contains_key("make_dbsnp_index") { task_set.insert("dbsnp_index"); }
	let mut task_list = Vec::new();
	for task in gem_bs.get_tasks_iter().filter(|t| t.command() == Command::Index) {
		if task_set.is_empty() || task_set.contains(task.id()) { task_list.push(task.idx()); }
	}
	for ix in task_list.iter() {
		let t = &gem_bs.get_tasks()[*ix];
		println!("{:?}", t);
	}
	
	Ok(())
}

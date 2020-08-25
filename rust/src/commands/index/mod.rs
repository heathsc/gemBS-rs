use clap::ArgMatches;
use std::collections::HashSet;
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::common::defs::{Section, Command};
use crate::common::{dry_run, utils};
use crate::scheduler;

pub fn index_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	// Get JSON config file from disk
	gem_bs.read_json_config()?;
	
	let options = handle_options(m, gem_bs, Section::Index);
	let task_path = gem_bs.get_task_file_path();
	let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?; 
	gem_bs.setup_assets_and_tasks(&flock)?;
	let mut task_set = HashSet::new();
	if options.contains_key("make_bs_index") { task_set.insert("index"); }
	if options.contains_key("make_nonbs_index") { task_set.insert("nonbs_index"); }
	if options.contains_key("make_dbsnp_index") { task_set.insert("dbsnp_index"); }
	let mut task_list = Vec::new();
	for task in gem_bs.get_tasks_iter().filter(|t| t.command() == Command::Index) {
		if task_set.is_empty() || task_set.contains(task.id()) { task_list.push(task.idx()); }
	}
	if gem_bs.dry_run() { dry_run::handle_dry_run(gem_bs, &options, &task_list) }
	if let Some(json_file) = gem_bs.json_out() { dry_run::handle_json_tasks(gem_bs, &options, &task_list, json_file)?; }
	if gem_bs.execute_flag() { scheduler::schedule_jobs(gem_bs, &options, &task_list, &[], &[], flock)?; }	
	
	Ok(())
}

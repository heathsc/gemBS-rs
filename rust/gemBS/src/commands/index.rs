use clap::ArgMatches;
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::common::defs::{Section, Command};
use crate::common::{dry_run, utils};
use crate::common::assets::GetAsset;
use crate::scheduler;

pub fn index_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	// Get config file from disk
	gem_bs.read_config()?;
	
	let options = handle_options(m, gem_bs, Section::Index);
	let task_path = gem_bs.get_task_file_path();
	let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?; 
	gem_bs.setup_assets_and_tasks(&flock)?;
	let mut asset_ids = Vec::new();
	let mut asset_names = Vec::new();
	if options.contains_key("make_bs_index") { asset_names.push("index") }
	if options.contains_key("make_nonbs_index") { asset_names.push("nonbs_index") }
	if options.contains_key("make_dbsnp_index") { 
		asset_names.push("dbsnp_index") 
	}
	let chk = if asset_names.is_empty() { 
		asset_names.extend_from_slice(&["index", "nonbs_index", "dbsnp_index"]);
		false
	} else { true };
	for s in asset_names.iter() {
		match gem_bs.get_asset(*s) {
			Some(asset) => asset_ids.push(asset.idx()),
			None => if chk { warn!("Index {} not required, option ignored", *s)},
		}
	}
	let com_set = [Command::Index];
	let task_list = gem_bs.get_required_tasks_from_asset_list(&asset_ids, &com_set);
	if gem_bs.execute_flag() { scheduler::schedule_jobs(gem_bs, &options, &task_list, &asset_ids, &com_set, flock) }	
	else { dry_run::handle_nonexec(gem_bs, &options, &task_list) }
}

use clap::ArgMatches;
use std::collections::{HashMap, HashSet};
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::common::assets::{AssetType};
use crate::common::defs::{Section, Command, DataValue};
use crate::common::{dry_run, utils};
use crate::scheduler;

fn collect_terminal_assets(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>) -> Result<Vec<usize>, String> {
	let barcodes = super::get_barcode_list(gem_bs, options)?;
	let mut flag = vec!(true; gem_bs.get_assets().len());	
	// First we mask out all assets that are requirements of other assets
	for asset in gem_bs.get_assets().iter() { asset.parents().iter().for_each(|x| flag[*x] = false); }

	let bc_hash = barcodes.iter().fold(HashSet::new(), |mut h, x| { h.insert(*x); h } );
	// Now filter out those that are not from tasks relating to barcode list
	for task in gem_bs.get_tasks().iter() {
		if if let Some(bc) = task.barcode() {
			! bc_hash.contains(bc) }
		else { false } { task.outputs().for_each(|x| flag[*x] = false) }
	}
	// Make final list
	let assets: Vec<_> = gem_bs.get_assets().iter().filter(|x| x.asset_type() == AssetType::Derived).map(|x| x.idx()).filter(|x| flag[*x]).collect();
	Ok(assets)
}

pub fn run_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	// Get JSON config file from disk
	gem_bs.read_json_config()?;
	
	let options = handle_options(m, gem_bs, Section::Index);
	let task_path = gem_bs.get_task_file_path();
	let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?; 
	gem_bs.setup_assets_and_tasks(&flock)?;
	let assets = collect_terminal_assets(gem_bs, &options)?;
	let com_set = [Command::Index, Command::Map, Command::MergeBams, Command::MergeCallJsons, Command::Call, Command::MergeBcfs, Command::Extract,
		Command::MapReport, Command::CallReport, Command::Report, Command::MD5Sum, Command::IndexBcf];
	let task_list = gem_bs.get_required_tasks_from_asset_list(&assets, &com_set);
	if options.contains_key("_dry_run") { dry_run::handle_dry_run(gem_bs, &options, &task_list) }
	if let Some(DataValue::String(json_file)) = options.get("_json") { dry_run::handle_json_tasks(gem_bs, &options, &task_list, json_file)?; }
	if !(options.contains_key("_dry_run") || options.contains_key("_json")) { scheduler::schedule_jobs(gem_bs, &options, &task_list, &assets, &com_set, flock)?; }		
	Ok(())
}

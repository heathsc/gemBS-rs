use clap::ArgMatches;
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::common::assets::{AssetType};
use crate::common::defs::{Section, Command};
use crate::common::{dry_run, utils};
use crate::scheduler;

fn collect_terminal_assets(gem_bs: &mut GemBS) -> Result<Vec<usize>, String> {
	let mut flag = vec!(true; gem_bs.get_assets().len());	
	// Mask out all assets that are requirements of other assets
	for asset in gem_bs.get_assets().iter() { asset.parents().iter().for_each(|x| flag[*x] = false); }
	// Make final list
	let assets: Vec<_> = gem_bs.get_assets().iter().filter(|x| x.asset_type() == AssetType::Derived).map(|x| x.idx()).filter(|x| flag[*x]).collect();
	Ok(assets)
}

pub fn run_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	// Get JSON config file from disk
	gem_bs.read_json_config()?;
	
	let options = handle_options(m, gem_bs, Section::Default);
	let task_path = gem_bs.get_task_file_path();
	let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?; 
	gem_bs.setup_assets_and_tasks(&flock)?;
	let assets = collect_terminal_assets(gem_bs)?;
	let com_set = [Command::Index, Command::Map, Command::MergeBams, Command::MergeCallJsons, Command::Call, Command::MergeBcfs, Command::Extract,
		Command::MD5SumMap, Command::MD5SumCall, Command::IndexBcf, Command::MapReport, Command::CallReport, Command::Report];
	let task_list = gem_bs.get_required_tasks_from_asset_list(&assets, &com_set);
	if gem_bs.execute_flag() { scheduler::schedule_jobs(gem_bs, &options, &task_list, &assets, &com_set, flock) }		
	else { dry_run::handle_nonexec(gem_bs, &options, &task_list) }
}

use clap::ArgMatches;
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::common::defs::{Section, Command, DataValue};
use crate::common::{dry_run, utils};
use crate::scheduler;

pub mod make_map_report;
pub mod make_call_report;
pub mod report_utils;

fn collect_assets(gem_bs: &GemBS, id: &str) -> Result<Vec<usize>, String> {
	if let Some(t) = gem_bs.get_tasks().find_task(id) {
		Ok(gem_bs.get_tasks()[t].outputs().copied().collect())
	} else { Err(format!("Couldn't find report task {}", id)) }
}

pub fn map_report_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	// Get JSON config file from disk
	gem_bs.read_json_config()?;
	
	let options = handle_options(m, gem_bs, Section::Report);
	let task_path = gem_bs.get_task_file_path();
	let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?; 
	gem_bs.setup_assets_and_tasks(&flock)?;
	let com_set = if gem_bs.all() { vec!(Command::Index, Command::Map, Command::MapReport) } else { vec!(Command::MapReport) };
	let assets = collect_assets(gem_bs, "map_report")?;
	let task_list = gem_bs.get_required_tasks_from_asset_list(&assets, &com_set);
	if options.contains_key("_dry_run") { dry_run::handle_dry_run(gem_bs, &options, &task_list) }
	if let Some(DataValue::String(json_file)) = options.get("_json") { dry_run::handle_json_tasks(gem_bs, &options, &task_list, json_file)?; }
	if !(options.contains_key("_dry_run") || options.contains_key("_json")) { scheduler::schedule_jobs(gem_bs, &options, &task_list, &assets, &com_set, flock)?; }		
	Ok(())
}

pub fn call_report_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	// Get JSON config file from disk
	gem_bs.read_json_config()?;
	
	let options = handle_options(m, gem_bs, Section::Report);
	let task_path = gem_bs.get_task_file_path();
	let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?; 
	gem_bs.setup_assets_and_tasks(&flock)?;
	let com_set = if gem_bs.all() { vec!(Command::Index, Command::Map, Command::MergeBams, Command::MergeCallJsons, Command::IndexBcf, Command::Call, Command::CallReport) } else { vec!(Command::CallReport, Command::MergeCallJsons) };
	let assets = collect_assets(gem_bs, "call_report")?;
	
	let task_list = gem_bs.get_required_tasks_from_asset_list(&assets, &com_set);
	if options.contains_key("_dry_run") { dry_run::handle_dry_run(gem_bs, &options, &task_list) }
	if let Some(DataValue::String(json_file)) = options.get("_json") { dry_run::handle_json_tasks(gem_bs, &options, &task_list, json_file)?; }
	if !(options.contains_key("_dry_run") || options.contains_key("_json")) { scheduler::schedule_jobs(gem_bs, &options, &task_list, &assets, &com_set, flock)?; }		
	Ok(())
}

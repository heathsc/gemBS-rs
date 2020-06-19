use std::collections::HashMap;
use clap::ArgMatches;
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::common::defs::{Section, Command, DataValue};
use crate::common::assets::GetAsset;
use crate::common::{dry_run, utils};
use super::call;
use crate::scheduler;

fn get_required_asset_list(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>) -> Result<Vec<usize>, String> {	
	let barcodes = call::get_barcode_list(gem_bs, options)?;
	let mut asset_list = Vec::new();
	for bc in barcodes.iter() {
		if gem_bs.get_config_bool(Section::Extract, "make_cpg") { asset_list.push(gem_bs.get_asset(format!("{}_cpg.txt.gz", bc).as_str()).unwrap().idx()) }
		if gem_bs.get_config_bool(Section::Extract, "make_non_cpg") { asset_list.push(gem_bs.get_asset(format!("{}_non_cpg.txt.gz", bc).as_str()).unwrap().idx()) }
		if gem_bs.get_config_bool(Section::Extract, "make_bedmthyl") { asset_list.push(gem_bs.get_asset(format!("{}_cpg.bed.gz", bc).as_str()).unwrap().idx()) }
		if gem_bs.get_config_bool(Section::Extract, "make_snps") { asset_list.push(gem_bs.get_asset(format!("{}_snps.txt.gz", bc).as_str()).unwrap().idx()) }
	}
	Ok(asset_list)
}

pub fn extract_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	gem_bs.read_json_config()?;
	
	let options = handle_options(m, gem_bs, Section::Extract);
	let task_path = gem_bs.get_task_file_path();
	let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?; 
	gem_bs.setup_assets_and_tasks(&flock)?;
	let asset_ids = get_required_asset_list(gem_bs, &options)?;
	let task_list = if gem_bs.all() { gem_bs.get_required_tasks_from_asset_list(&asset_ids, &[Command::Index, Command::Map, Command::MergeBams, Command::Call, Command::MergeBcfs, Command::Extract]) }
		else { gem_bs.get_required_tasks_from_asset_list(&asset_ids, &[Command::Extract]) };
	if options.contains_key("_dry_run") { dry_run::handle_dry_run(gem_bs, &options, &task_list) }
	if let Some(DataValue::String(json_file)) = options.get("_json") { dry_run::handle_json_tasks(gem_bs, &options, &task_list, json_file)?; }
	if !(options.contains_key("_dry_run") || options.contains_key("_json")) { scheduler::schedule_jobs(gem_bs, &options, &task_list, flock)?; }	
	Ok(())
}

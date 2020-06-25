use clap::ArgMatches;
use std::collections::{HashMap, HashSet};
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::common::assets::GetAsset;
use crate::common::defs::{Section, Command, DataValue};
use crate::common::{dry_run, utils};
use crate::scheduler;

pub fn get_assets_md5_map(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>, assets: &mut HashSet<usize>, coms: &mut HashSet<Command>) -> Result<(), String> {
	let barcodes = super::get_barcode_list(gem_bs, options)?;
	let suffix = if gem_bs.get_config_bool(Section::Mapping, "make_cram") { "cram" } else { "bam" };	
	for bc in barcodes {
		let id = format!("{}.{}.md5", bc, suffix);
		if let Some(asset) = gem_bs.get_asset(id.as_str()) { assets.insert(asset.idx()); }	
		else { return Err(format!("Unknown barcode {}", bc)); }
	}
	coms.insert(Command::MD5Sum);
	if gem_bs.all() { [Command::Index, Command::Map, Command::MergeBams].iter().for_each(|x| {coms.insert(*x);}) }	
	Ok(())
}

pub fn get_assets_md5_call(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>, assets: &mut HashSet<usize>, coms: &mut HashSet<Command>) -> Result<(), String> {
	let barcodes = super::get_barcode_list(gem_bs, options)?;
	for bc in barcodes {
		let id = format!("{}.bcf.md5", bc);
		if let Some(asset) = gem_bs.get_asset(id.as_str()) { assets.insert(asset.idx()); }	
		else { return Err(format!("Unknown barcode {}", bc)); }
	}
	coms.insert(Command::MD5Sum);
	if gem_bs.all() { [Command::Index, Command::Map, Command::MergeBams, Command::Call, Command::MergeBcfs].iter().for_each(|x| {coms.insert(*x);}) }	
	Ok(())
}

pub fn md5sum_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	// Get JSON config file from disk
	gem_bs.read_json_config()?;
	
	let options = handle_options(m, gem_bs, Section::Index);
	let task_path = gem_bs.get_task_file_path();
	let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?; 
	gem_bs.setup_assets_and_tasks(&flock)?;
	let mut assets = HashSet::new();
	let mut coms = HashSet::new();
	if let Some(x) = m.value_of("scope") {
		match x.to_lowercase().as_str() {
			"map" => get_assets_md5_map(gem_bs, &options, &mut assets, &mut coms)?,
			"call" => get_assets_md5_call(gem_bs, &options, &mut assets, &mut coms)?,
			_ => return Err(format!("Unknown scope {} for md5sum subcommand", x)),
		}
	} else {
		get_assets_md5_map(gem_bs, &options, &mut assets, &mut coms)?;
		get_assets_md5_call(gem_bs, &options, &mut assets, &mut coms)?;
	}
	let asset_ids: Vec<_> = assets.iter().copied().collect();
	let com_set: Vec<_> = coms.iter().copied().collect();
	let task_list = gem_bs.get_required_tasks_from_asset_list(&asset_ids, &com_set);
	if options.contains_key("_dry_run") { dry_run::handle_dry_run(gem_bs, &options, &task_list) }
	if let Some(DataValue::String(json_file)) = options.get("_json") { dry_run::handle_json_tasks(gem_bs, &options, &task_list, json_file)?; }
	if !(options.contains_key("_dry_run") || options.contains_key("_json")) { scheduler::schedule_jobs(gem_bs, &options, &task_list, &asset_ids, &com_set, flock)?; }		
	Ok(())
}
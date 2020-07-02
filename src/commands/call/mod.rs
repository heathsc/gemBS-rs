use std::collections::{HashSet, HashMap};
use clap::ArgMatches;
use crate::cli::utils::handle_options;
use crate::config::{GemBS, contig};
use crate::common::defs::{Section, Command, DataValue};
use crate::common::assets::GetAsset;
use crate::common::dry_run;
use crate::common::utils;
use crate::scheduler;

fn get_required_asset_list(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>) -> Result<HashSet<usize>, String> {
	
	let barcodes = super::get_barcode_list(gem_bs, options)?;
	let pools = contig::get_contig_pools(gem_bs);
	
	// First we add the merged BCFs if required
	let mut asset_ids = HashSet::new();	
	if !options.contains_key("_no_merge") || pools.len() == 1 {
		if !options.contains_key("_index") {
			for barcode in barcodes.iter() {
				if let Some(asset) = gem_bs.get_asset(format!("{}.bcf", barcode).as_str()) { asset_ids.insert(asset.idx()); }
				else { return Err(format!("Unknown barcode {}", barcode)) }
				asset_ids.insert(gem_bs.get_asset(format!("{}_call.json", barcode).as_str()).expect("Couldn't get call JSON asset").idx());
			}
		}
		if !options.contains_key("_no_index") {
			for barcode in barcodes.iter() {
				asset_ids.insert(gem_bs.get_asset(format!("{}.bcf.csi", barcode).as_str()).expect("Couldn't get bcf index asset").idx());
			}
		}
	}
	// Now the individual contig pools
	if !options.contains_key("_merge") && !options.contains_key("_index") && pools.len() > 1 {
		let add_bcf_asset = |b: &str, p: &str, rf: &mut HashSet<usize>| {
			if let Some(asset) = gem_bs.get_asset(format!("{}_{}.bcf", b, p).as_str()) { rf.insert(asset.idx()); }
			else { return Err(format!("Unknown pool {}", p)); }
			if let Some(asset) = gem_bs.get_asset(format!("{}_{}_call.json", b, p).as_str()) { rf.insert(asset.idx()); Ok(()) }
			else { Err("Couldn't get pool JSON asset".to_string()) }
		};
		let add_pool_asset = |p: &str, rf: &mut HashSet<usize>| -> Result<(), String> {
			for barcode in barcodes.iter() { add_bcf_asset(barcode, p, rf)? }
			Ok(())
		};
		if let Some(DataValue::StringVec(vpool)) = options.get("_pool") { for pool in vpool.iter() { add_pool_asset(pool, &mut asset_ids)? }}
		else { for pool in pools.iter() { add_pool_asset(pool, &mut asset_ids)?; }}
	}
	Ok(asset_ids)
}

fn gen_call_command(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>) -> Result<(), String> {
	let task_path = gem_bs.get_task_file_path();
	let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?; 
	gem_bs.setup_assets_and_tasks(&flock)?;
	let mut assets = get_required_asset_list(gem_bs, &options)?;
	let mut coms = HashSet::new();
	if !options.contains_key("_no_md5") { super::md5sum::get_assets_md5_call(gem_bs, &options, &mut assets, &mut coms)?; }
	if gem_bs.all() { [Command::Index, Command::Map, Command::MergeBams, Command::Call].iter().for_each(|x| { coms.insert(*x); }) }
	else if !(options.contains_key("_merge") || options.contains_key("_index")) { coms.insert(Command::Call); }
	if !(options.contains_key("_no_merge") || options.contains_key("_index")) { 
		coms.insert(Command::MergeBcfs); 
		coms.insert(Command::MergeCallJsons); 
	}
	if !options.contains_key("_no_index") { coms.insert(Command::IndexBcf); }
	let asset_ids: Vec<_> = assets.iter().copied().collect();
	let com_set: Vec<_> = coms.iter().copied().collect();
	let task_list = gem_bs.get_required_tasks_from_asset_list(&asset_ids, &com_set);
	if options.contains_key("_dry_run") { dry_run::handle_dry_run(gem_bs, &options, &task_list); }
	if let Some(DataValue::String(json_file)) = options.get("_json") { dry_run::handle_json_tasks(gem_bs, &options, &task_list, json_file)?; }
	if !(options.contains_key("_dry_run") || options.contains_key("_json")) { scheduler::schedule_jobs(gem_bs, &options, &task_list, &asset_ids, &com_set, flock)?; }	
	Ok(())
}

pub fn call_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	gem_bs.read_json_config()?;
	
	let mut options = handle_options(m, gem_bs, Section::Calling);
	if options.contains_key("_pool") { options.insert("_no_merge", DataValue::Bool(true)); }
	if options.contains_key("_no_merge") {
		 options.insert("_no_md5", DataValue::Bool(true)); 		
		 options.insert("_no_index", DataValue::Bool(true)); 
	}
	gen_call_command(gem_bs, &options)
}

pub fn merge_bcfs_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	gem_bs.read_json_config()?;
	
	let mut options = handle_options(m, gem_bs, Section::Calling);
	options.insert("_merge", DataValue::Bool(true));	
	gen_call_command(gem_bs, &options)
}

pub fn index_bcf_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	gem_bs.read_json_config()?;

	let mut options = handle_options(m, gem_bs, Section::Calling);
	options.insert("_index", DataValue::Bool(true));	
	gen_call_command(gem_bs, &options)
}


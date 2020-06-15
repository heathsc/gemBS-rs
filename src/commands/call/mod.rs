use std::collections::{HashMap, HashSet};
use clap::ArgMatches;
use crate::cli::utils::handle_options;
use crate::config::{GemBS, contig};
use crate::common::defs::{Section, Command, DataValue, Metadata};
use crate::common::assets::GetAsset;
use crate::common::dry_run;

pub fn get_barcode_list<'a>(gem_bs: &'a GemBS, options: &'a HashMap<&'static str, DataValue>) -> Result<Vec<&'a String>, String> {
	let mut barcodes = Vec::new();
	if let Some(DataValue::String(barcode)) = options.get("_barcode") { barcodes.push(barcode); }
	else if let Some(DataValue::String(sample)) = options.get("_sample") {
		for hr in gem_bs.get_sample_data_ref().values() {
			if let Some(DataValue::String(x)) = hr.get(&Metadata::SampleName) {
				if x == sample {
					if let Some(DataValue::String(barcode)) = hr.get(&Metadata::SampleBarcode) {	
						barcodes.push(barcode);
						break;
					}
				}
			}
		}
		if barcodes.is_empty() { return Err(format!("Unknown sample {}", sample)) } 
	} else {
		let mut samples = HashSet::new();
		for hr in gem_bs.get_sample_data_ref().values() {
			if let Some(DataValue::String(bc)) = hr.get(&Metadata::SampleBarcode) { samples.insert(bc); }
		}
		samples.iter().for_each(|x| barcodes.push(x));
	}
	Ok(barcodes)	
}
fn get_required_asset_list(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>) -> Result<Vec<usize>, String> {
	
	let barcodes = get_barcode_list(gem_bs, options)?;
	let pools = contig::get_contig_pools(gem_bs);
	
	// First we add the merged BCFs if required
	let mut asset_ids = Vec::new();	
	if !options.contains_key("no_merge") && pools.len() > 1 {
		for barcode in barcodes.iter() {
			if let Some(asset) = gem_bs.get_asset(format!("{}.bcf", barcode).as_str()) { asset_ids.push(asset.idx()) }
			else { return Err(format!("Unknown barcode {}", barcode)) }
		}	
	}
	// Now the individual contigs pools
	if !options.contains_key("_merge") && pools.len() > 1 {
		let add_bcf_asset = |b: &str, p: &str, rf: &mut Vec<usize>| {
			if let Some(asset) = gem_bs.get_asset(format!("{}_{}.bcf", b, p).as_str()) { rf.push(asset.idx()); Ok(()) }
			else { Err(format!("Unknown pool {}", p)) }
		};
		let add_pool_asset = |p: &str, rf: &mut Vec<usize>| -> Result<(), String> {
			for barcode in barcodes.iter() { add_bcf_asset(barcode, p, rf)? }
			Ok(())
		};
		if let Some(DataValue::StringVec(vpool)) = options.get("_pool") { for pool in vpool.iter() { add_pool_asset(pool, &mut asset_ids)? }}
		else { for pool in pools.iter() { add_pool_asset(pool, &mut asset_ids)?; }}
	}
	Ok(asset_ids)
}

fn gen_call_command(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>) -> Result<(), String> {
	gem_bs.setup_assets_and_tasks()?;
	let asset_ids = get_required_asset_list(gem_bs, &options)?;
	let mut com_set = Vec::new();
	if gem_bs.all() { [Command::Index, Command::Map, Command::MergeBams, Command::Call].iter().for_each(|x| com_set.push(*x)) }
	else if !options.contains_key("_merge") { com_set.push(Command::Call); }
	if !options.contains_key("no_merge") { com_set.push(Command::MergeBcfs); }
	let task_list = gem_bs.get_required_tasks_from_asset_list(&asset_ids, &com_set);
	if options.contains_key("_dry_run") { dry_run::handle_dry_run(gem_bs, &options, &task_list) }
	if let Some(DataValue::String(json_file)) = options.get("_json") { dry_run::handle_json_tasks(gem_bs, &options, &task_list, json_file)?; }
	Ok(())
}

pub fn call_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	gem_bs.read_json_config()?;
	
	let mut options = handle_options(m, gem_bs, Section::Calling);
	if options.contains_key("_pool") { options.insert("no_merge", DataValue::Bool(true)); }
	gen_call_command(gem_bs, &options)
}

pub fn merge_bcfs_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	gem_bs.read_json_config()?;
	
	let mut options = handle_options(m, gem_bs, Section::Calling);
	options.insert("_merge", DataValue::Bool(true));	
	gen_call_command(gem_bs, &options)
}

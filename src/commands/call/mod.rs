use std::collections::{HashMap, HashSet};
use clap::ArgMatches;
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::config::contig;
use crate::common::defs::{Section, Command, DataValue, Metadata};
use crate::common::assets::GetAsset;


fn get_barcode_list<'a>(gem_bs: &'a GemBS, options: &'a HashMap<&'static str, DataValue>) -> Result<Vec<&'a String>, String> {
	let mut barcodes = Vec::new();
	if let Some(DataValue::String(barcode)) = options.get("barcode") { barcodes.push(barcode); }
	else if let Some(DataValue::String(sample)) = options.get("sample") {
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
	if !options.contains_key("merge") && pools.len() > 1 {
		let add_bcf_asset = |b: &str, p: &str, rf: &mut Vec<usize>| {
			if let Some(asset) = gem_bs.get_asset(format!("{}_{}.bcf", b, p).as_str()) { rf.push(asset.idx()); Ok(()) }
			else { Err(format!("Unknown pool {}", p)) }
		};
		let add_pool_asset = |p: &str, rf: &mut Vec<usize>| -> Result<(), String> {
			for barcode in barcodes.iter() { add_bcf_asset(barcode, p, rf)? }
			Ok(())
		};
		if let Some(DataValue::StringVec(vpool)) = options.get("pool") { for pool in vpool.iter() { add_pool_asset(pool, &mut asset_ids)? }}
		else { for pool in pools.iter() { add_pool_asset(pool, &mut asset_ids)?; }}
	}
	Ok(asset_ids)
}

fn gen_call_command(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>) -> Result<(), String> {
	gem_bs.setup_assets_and_tasks()?;
	let asset_ids = get_required_asset_list(gem_bs, &options)?;
	let task_list = gem_bs.get_required_tasks_from_asset_list(&asset_ids);
	let mut com_set = HashSet::new();
	if !options.contains_key("merge") { com_set.insert(Command::Call); }
	if !options.contains_key("no_merge") { com_set.insert(Command::MergeBcfs); }
			
	for ix in task_list.iter() {
		let t = &gem_bs.get_tasks()[*ix];
		if com_set.contains(&t.command()) {
			println!("{:?} {:?}", t, gem_bs.task_status(t));
		}
	}
	Ok(())
}

pub fn call_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	gem_bs.read_json_config()?;
	
	let mut options: HashMap<&'static str, DataValue> = HashMap::new();
	handle_options(m, gem_bs, Section::Calling, &mut options);
	if options.contains_key("pool") { options.insert("no_merge", DataValue::Bool(true)); }
	gen_call_command(gem_bs, &options)
}

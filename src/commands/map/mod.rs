use std::collections::{HashMap, HashSet};
use clap::ArgMatches;
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::common::defs::{Section, Command, DataValue, Metadata};
use crate::common::assets::GetAsset;

fn get_required_asset_list(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>) -> Result<Vec<usize>, String> {

	let make_cram = gem_bs.get_config_bool(Section::Mapping, "make_cram");
	let suffix = if make_cram { "cram" } else { "bam" };
	let mut asset_ids = Vec::new();
	if let Some(DataValue::String(dataset)) = options.get("dataset") {
		if let Some(asset) = gem_bs.get_asset(format!("{}.bam", dataset).as_str()).or_else(|| {
			if let Some(DataValue::String(bc)) = gem_bs.get_sample_data_ref().get(dataset).and_then(|rf| rf.get(&Metadata::SampleBarcode)) {
				gem_bs.get_asset(format!("{}.{}", bc, suffix).as_str())
			} else { None }
		}) { asset_ids.push(asset.idx()) } else { return Err(format!("Unknown dataset {}", dataset)) }	
	} else if let Some(DataValue::String(barcode)) = options.get("barcode") {
		if let Some(asset) = gem_bs.get_asset(format!("{}.{}", barcode, suffix).as_str()) { asset_ids.push(asset.idx()) }
		else { return Err(format!("Unknown barcode {}", barcode)) }	
	} else if let Some(DataValue::String(sample)) = options.get("sample") {
		let mut asset = None;
		for hr in gem_bs.get_sample_data_ref().values() {
			if let Some(DataValue::String(x)) = hr.get(&Metadata::SampleName) {
				if x == sample {
					if let Some(DataValue::String(bc)) = hr.get(&Metadata::SampleBarcode) {	
						asset = gem_bs.get_asset(format!("{}.{}", bc, suffix).as_str());
						break;
					}
				}
			}
		}
		if let Some(a) = asset { asset_ids.push(a.idx())} else { return Err(format!("Unknown sample {}", sample)) }	
	} else {
		let mut samples = HashSet::new();
		for hr in gem_bs.get_sample_data_ref().values() {
			if let Some(DataValue::String(bc)) = hr.get(&Metadata::SampleBarcode) { samples.insert(bc); }
		}
		for bc in samples.iter() {
			if let Some(asset) = gem_bs.get_asset(format!("{}.{}", bc, suffix).as_str()) { asset_ids.push(asset.idx()) }
			else { return Err(format!("Missing asset for barcode {}", bc)) }			
		}
	}
	Ok(asset_ids)
}

fn gen_map_command(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>) -> Result<(), String> {
	gem_bs.setup_assets_and_tasks()?;
	let asset_ids = get_required_asset_list(gem_bs, &options)?;
	let task_list = gem_bs.get_required_tasks_from_asset_list(&asset_ids);
	let mut com_set = HashSet::new();
	if !options.contains_key("merge") { com_set.insert(Command::Map); }
	if !options.contains_key("no_merge") { com_set.insert(Command::MergeBams); }
			
	for ix in task_list.iter() {
		let t = &gem_bs.get_tasks()[*ix];
		if com_set.contains(&t.command()) {
			println!("{:?} {:?}", t, gem_bs.task_status(t));
		}
	}
	Ok(())
}

pub fn map_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	gem_bs.read_json_config()?;
	
	let mut options: HashMap<&'static str, DataValue> = HashMap::new();
	handle_options(m, gem_bs, Section::Mapping, &mut options);
	gen_map_command(gem_bs, &options)
}

pub fn merge_bams_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	gem_bs.read_json_config()?;
	
	let mut options: HashMap<&'static str, DataValue> = HashMap::new();
	handle_options(m, gem_bs, Section::Mapping, &mut options);
	options.insert("merge", DataValue::Bool(true));
	gen_map_command(gem_bs, &options)
}

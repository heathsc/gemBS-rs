use std::collections::{HashMap, HashSet};
use crate::config::GemBS;
use crate::common::assets::GetAsset;
use crate::common::defs::{Section, Command, DataValue};

pub fn get_assets_md5_map(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>, assets: &mut HashSet<usize>, coms: &mut HashSet<Command>) -> Result<(), String> {
	let barcodes = super::get_barcode_list(gem_bs, options)?;
	let suffix = if gem_bs.get_config_bool(Section::Mapping, "make_cram") { "cram" } else { "bam" };	
	for bc in barcodes {
		let id = format!("{}.{}.md5", bc, suffix);
		if let Some(asset) = gem_bs.get_asset(id.as_str()) { assets.insert(asset.idx()); }	
		else { return Err(format!("Unknown barcode {}", bc)); }
	}
	coms.insert(Command::MD5SumMap);
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
	coms.insert(Command::MD5SumCall);
	if gem_bs.all() { [Command::Index, Command::Map, Command::MergeBams, Command::Call, Command::MergeBcfs].iter().for_each(|x| {coms.insert(*x);}) }	
	Ok(())
}

// Check requirements and presence of source and derived files for map report
// Make asset list for BCFs, BED, BigWig etc. associated with map_report

use std::path::{Path, PathBuf};
use crate::common::defs::{Section, DataValue, Command, Metadata};
use crate::common::assets::{AssetType, GetAsset};
use super::GemBS;

pub fn check_map_report(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Report, name ) { x } else { "gemBS_reports" } };
	let rdir = Path::new(get_dir("report_dir"));
	let report_dir: PathBuf = [rdir, Path::new("mapping")].iter().collect(); 
	let cores = gem_bs.get_config_int(Section::Report, "cores").map(|x| x as usize).or_else(|| Some(2));
	let memory = gem_bs.get_config_memsize(Section::Report, "memory");
	let time = gem_bs.get_config_joblen(Section::Report, "time").or_else(|| Some(3600.into()));

	let handle_file = |gb: &mut GemBS, id: &str, nm: &str, p: &Path| {
		let path: PathBuf = [p, Path::new(nm)].iter().collect();
		gb.insert_asset(id, &path, AssetType::Derived)
	}; 
	let samples = gem_bs.get_samples();
	let mut json_files = Vec::new();
	let mut out_vec = Vec::new();
	out_vec.push(handle_file(gem_bs, "map_report_index.html", "index.html", &report_dir));
	out_vec.push(handle_file(gem_bs, "map_report_style.css", "style.css", &report_dir));
	for (bc, _) in samples.iter() { 
		let bc_dir: PathBuf = [&report_dir, Path::new(bc)].iter().collect();
		out_vec.push(handle_file(gem_bs, format!("{}_map_index.html", bc).as_str(), "index.html", &bc_dir));
		out_vec.push(handle_file(gem_bs, format!("{}_isize.png", bc).as_str(), format!("{}_isize.png", bc).as_str(), &bc_dir));
		out_vec.push(handle_file(gem_bs, format!("{}_mapq.png", bc).as_str(), format!("{}_mapq.png", bc).as_str(), &bc_dir));
		json_files.extend(gem_bs.get_mapping_json_files_for_barcode(bc)); 
	}
	let mut dsets = Vec::new();
	let href = gem_bs.get_sample_data_ref();	
	for (dataset, href1) in href.iter() {
		if let Some(DataValue::String(bc)) = href1.get(&Metadata::SampleBarcode) { dsets.push((bc.to_owned(), dataset.to_owned())) }
		else { panic!("No barcode associated with dataset {}", dataset); }
	}
	for (bc, dset) in dsets.iter() {		
		let bc_dir: PathBuf = [&report_dir, Path::new(bc)].iter().collect();
		out_vec.push(handle_file(gem_bs, format!("{}.html", dset).as_str(), format!("{}.html", dset).as_str(), &bc_dir));
		out_vec.push(handle_file(gem_bs, format!("{}_isize.png", dset).as_str(), format!("{}_isize.png", dset).as_str(), &bc_dir));
		out_vec.push(handle_file(gem_bs, format!("{}_mapq.png", dset).as_str(), format!("{}_mapq.png", dset).as_str(), &bc_dir));
	}	
	let task = gem_bs.add_task("map_report", "Generate mapping report", Command::MapReport, "");
	gem_bs.add_task_inputs(task, &json_files).add_outputs(&out_vec).add_cores(cores).add_memory(memory).add_time(time);
	out_vec.iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &json_files));
	
	Ok(())
}

pub fn check_call_report(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Report, name ) { x } else { "gemBS_reports" } };
	let rdir = Path::new(get_dir("report_dir"));
	let report_dir: PathBuf = [rdir, Path::new("variant_calling")].iter().collect(); 
	let cores = gem_bs.get_config_int(Section::Report, "cores").map(|x| x as usize).or_else(|| Some(2));
	let memory = gem_bs.get_config_memsize(Section::Report, "memory");
	let time = gem_bs.get_config_joblen(Section::Report, "time").or_else(|| Some(3600.into()));

	let handle_file = |gb: &mut GemBS, id: &str, nm: &str, p: &Path| {
		let path: PathBuf = [p, Path::new(nm)].iter().collect();
		gb.insert_asset(id, &path, AssetType::Derived)
	}; 
	let samples = gem_bs.get_samples();
	let mut json_files = Vec::new();
	let mut out_vec = Vec::new();
	out_vec.push(handle_file(gem_bs, "variant_report_index.html", "index.html", &report_dir));
	out_vec.push(handle_file(gem_bs, "variant_report_style.css", "style.css", &report_dir));
	for (bc, _) in samples.iter() { 
		let bc_dir: PathBuf = [&report_dir, Path::new(bc)].iter().collect();
		let id = format!("{}_mapping.html", bc);
		out_vec.push(handle_file(gem_bs, &id, &id, &bc_dir));
		let id = format!("{}_methylation.html", bc);
		out_vec.push(handle_file(gem_bs, &id, &id, &bc_dir));
		let id = format!("{}_variants.html", bc);
		out_vec.push(handle_file(gem_bs, &id, &id, &bc_dir));
		json_files.push(gem_bs.get_asset(format!("{}_call.json", bc).as_str()).expect("Couldn't fine call JSON asset for call report").idx());
	}
	let task = gem_bs.add_task("variant_report", "Generate variant report", Command::CallReport, "");
	gem_bs.add_task_inputs(task, &json_files).add_outputs(&out_vec).add_cores(cores).add_memory(memory).add_time(time);
	out_vec.iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &json_files));
	
	Ok(())
}
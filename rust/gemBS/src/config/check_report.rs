// Check requirements and presence of source and derived files for map report
// Make asset list for BCFs, BED, BigWig etc. associated with map_report

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::str::FromStr;

use crate::common::defs::{Section, DataValue, Command, Metadata};
use crate::common::assets::{AssetType, GetAsset};
use super::GemBS;

pub fn check_map_report(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Report, name ) { x } else { "gemBS_reports" } };
	let rdir = Path::new(get_dir("report_dir"));
	let report_dir: PathBuf = [rdir, Path::new("mapping")].iter().collect(); 
	let css_dir: PathBuf = [rdir, Path::new("css")].iter().collect(); 
	let cores = gem_bs.get_config_int(Section::Report, "cores").map(|x| x as usize).or(Some(2));
	let memory = gem_bs.get_config_memsize(Section::Report, "memory");
	let time = gem_bs.get_config_joblen(Section::Report, "time").or_else(|| Some(3600.into()));

	let handle_file = |gb: &mut GemBS, id: &str, nm: &str, p: &Path| {
		let path: PathBuf = [p, Path::new(nm)].iter().collect();
		gb.insert_asset(id, &path, AssetType::Derived)
	}; 
	let samples = gem_bs.get_samples();
	let mut json_files = Vec::new();
	let mut out_vec = Vec::new();
	out_vec.push(handle_file(gem_bs, "map_report.tex", "map_report.tex", &report_dir));
	out_vec.push(handle_file(gem_bs, "map_report_index.html", "index.html", &report_dir));
	out_vec.push(handle_file(gem_bs, "style.css", "style.css", &css_dir));
	for (bc, _) in samples.iter() { 
		let bc_dir: PathBuf = [&report_dir, Path::new(bc)].iter().collect();
		let img_dir: PathBuf = [&report_dir, Path::new(bc), Path::new("images")].iter().collect();
		out_vec.push(handle_file(gem_bs, format!("{}_map_index.html", bc).as_str(), format!("{}.html", bc).as_str(), &bc_dir));
		out_vec.push(handle_file(gem_bs, format!("{}_isize.png", bc).as_str(), format!("{}_isize.png", bc).as_str(), &img_dir));
		out_vec.push(handle_file(gem_bs, format!("{}_mapq.png", bc).as_str(), format!("{}_mapq.png", bc).as_str(), &img_dir));
		json_files.extend(gem_bs.get_mapping_json_files_for_barcode(bc));		 
	}
	let mut dsets = Vec::new();
	let mut bc_count = HashMap::new();
	let href = gem_bs.get_sample_data_ref();	
	for (dataset, href1) in href.iter() {
		if let Some(DataValue::String(bc)) = href1.get(&Metadata::SampleBarcode) { 
			dsets.push((bc.to_owned(), dataset.to_owned()));
			*(bc_count.entry(bc.to_owned()).or_insert(0)) += 1;

		} else { panic!("No barcode associated with dataset {}", dataset); }
	}
	for (bc, dset) in dsets.iter() {
		if *bc_count.get(bc).expect("No count found for barcode") > 1 {
			let bc_dir: PathBuf = [&report_dir, Path::new(bc)].iter().collect();
			let img_dir: PathBuf = [&report_dir, Path::new(bc), Path::new("images")].iter().collect();
			out_vec.push(handle_file(gem_bs, format!("{}.html", dset).as_str(), format!("{}.html", dset).as_str(), &bc_dir));
			out_vec.push(handle_file(gem_bs, format!("{}_mapq.png", dset).as_str(), format!("{}_mapq.png", dset).as_str(), &img_dir));
			out_vec.push(handle_file(gem_bs, format!("{}_isize.png", dset).as_str(), format!("{}_isize.png", dset).as_str(), &img_dir));
		}
	}	
	let task = gem_bs.add_task("map_report", "Generate mapping report", Command::MapReport, "");
	gem_bs.add_task_inputs(task, &json_files).add_outputs(&out_vec).add_cores(cores).add_memory(memory).add_time(time);
	out_vec.iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &json_files));
	
	Ok(())
}

pub fn check_call_report(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Report, name ) { x } else { "gemBS_reports" } };
	let rdir = Path::new(get_dir("report_dir"));
	let report_dir: PathBuf = [rdir, Path::new("calling")].iter().collect(); 
	let cores = gem_bs.get_config_int(Section::Report, "cores").map(|x| x as usize).or(Some(2));
	let memory = gem_bs.get_config_memsize(Section::Report, "memory");
	let time = gem_bs.get_config_joblen(Section::Report, "time").or_else(|| Some(3600.into()));

	let handle_file = |gb: &mut GemBS, id: &str, nm: &str, p: &Path| {
		let path: PathBuf = [p, Path::new(nm)].iter().collect();
		gb.insert_asset(id, &path, AssetType::Derived)
	}; 
	let samples = gem_bs.get_samples();
	let mut json_files = Vec::new();
	let mut out_vec = Vec::new();
	out_vec.push(handle_file(gem_bs, "call_report.tex", "call_report.tex", &report_dir));
	out_vec.push(handle_file(gem_bs, "call_report_index.html", "index.html", &report_dir));
	for (bc, _) in samples.iter() { 
		let bc_dir: PathBuf = [&report_dir, Path::new(bc)].iter().collect();
		let img_dir: PathBuf = [&report_dir, Path::new(bc), Path::new("images")].iter().collect();
		let id = format!("{}_mapping_coverage.html", bc);
		out_vec.push(handle_file(gem_bs, &id, &id, &bc_dir));
		let id = format!("{}_methylation.html", bc);
		out_vec.push(handle_file(gem_bs, &id, &id, &bc_dir));
		let id = format!("{}_variants.html", bc);
		out_vec.push(handle_file(gem_bs, &id, &id, &bc_dir));
		for name in &[
			"coverage_all", "coverage_ref_cpg", "coverage_ref_cpg_inf", "coverage_non_ref_cpg", "coverage_non_ref_cpg_inf", "coverage_variants",
			"quality_all", "quality_ref_cpg", "quality_non_ref_cpg","quality_variants", "qd_variant", "qd_nonvariant", 
			"rmsmq_variant", "rmsmq_nonvariant", "fs_variant", "gc_coverage", "methylation_levels", "non_cpg_read_profile"
		] {
			let id = format!("{}_{}.png", bc, name);
			out_vec.push(handle_file(gem_bs, &id, &id, &img_dir));
			
		}
		json_files.push(gem_bs.get_asset(format!("{}_call.json", bc).as_str()).expect("Couldn't find call JSON asset for call report").idx());
	}
	let task = gem_bs.add_task("call_report", "Generate call report", Command::CallReport, "");
	gem_bs.add_task_inputs(task, &json_files).add_outputs(&out_vec).add_cores(cores).add_memory(memory).add_time(time);
	out_vec.iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &json_files));
	
	Ok(())
}

pub fn check_report(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Report, name ) { x } else { "gemBS_reports" } };
	let rdir = PathBuf::from_str(get_dir("report_dir")).map_err(|e| format!("{}", e))?;
	debug!("Report dir: {}", rdir.display());
	let project = gem_bs.get_config_str(Section::Report, "project");
	let cores = Some(1);
	let memory = gem_bs.get_config_memsize(Section::Report, "memory");
	let time = gem_bs.get_config_joblen(Section::Report, "time").or_else(|| Some(3600.into()));
	let mut in_vec = Vec::new();
	in_vec.push(gem_bs.get_asset("map_report_index.html").expect("Couldn't find map report index asset").idx());
	in_vec.push(gem_bs.get_asset("map_report.tex").expect("Couldn't find map report latex asset").idx());
	in_vec.push(gem_bs.get_asset("call_report_index.html").expect("Couldn't find call report index asset").idx());
	in_vec.push(gem_bs.get_asset("call_report.tex").expect("Couldn't find call report latex asset").idx());
	let handle_file = |gb: &mut GemBS, id: &str, nm: String, p: &Path| {
		let path: PathBuf = [p, Path::new(&nm)].iter().collect();
		gb.insert_asset(id, &path, AssetType::Derived)
	}; 
	let mut out_vec = Vec::new();
	let name = if let Some(s) = project { format!("{}_QC_Report", s) } else { "GemBS_QC_Report".to_string() };
	out_vec.push(handle_file(gem_bs, "report.tex", format!("{}.tex", name), &rdir));
	out_vec.push(handle_file(gem_bs, "report.html", format!("{}.html", name), &rdir));
	if gem_bs.get_config_bool(Section::Report, "pdf") {
		out_vec.push(handle_file(gem_bs, "report.pdf", format!("{}.pdf", name), &rdir));
	}
	let task = gem_bs.add_task("report", "Generate report", Command::Report, "");
	gem_bs.add_task_inputs(task, &in_vec).add_outputs(&out_vec).add_cores(cores).add_memory(memory).add_time(time);
	out_vec.iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &in_vec));
	Ok(())
}
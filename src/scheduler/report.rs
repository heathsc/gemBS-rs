use std::path::PathBuf;
use std::collections::HashMap; 
use std::io::BufRead;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use crate::config::GemBS;
use crate::common::defs::{Section, Metadata, DataValue};
use crate::common::assets::GetAsset;
use crate::common::json_call_stats::CallJson;
use super::{QPipe, QPipeCom};
use crate::common::compress;
use crate::common::utils;

#[derive(Debug)]
pub struct SampleJsonFiles {
	barcode: String,
	json_files: Vec<(String, PathBuf)>,
}

pub fn make_map_report_pipeline(gem_bs: &GemBS, job: usize) -> QPipe
{
	let task = &gem_bs.get_tasks()[job];
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	let project = gem_bs.get_config_str(Section::Report, "project").map(|x| x.to_owned());
	for out in task.outputs() { pipeline.add_outputs(gem_bs.get_asset(*out).expect("Couldn't get map-report output asset").path()); }
	let href = gem_bs.get_sample_data_ref();	
	let mut bcodes = HashMap::new();	
	for (dataset, href1) in href.iter() {
		if let Some(DataValue::String(bc)) = href1.get(&Metadata::SampleBarcode) { 
			bcodes.entry(bc.to_owned()).or_insert_with(Vec::new).push(dataset.to_owned());
		} else { panic!("No barcode associated with dataset {}", dataset); }
	}
	let mut json_files = Vec::new();
	for(bc, dvec) in bcodes.iter() {
		let v = if dvec.len() == 1 {
			let json = gem_bs.get_asset(format!("{}_map.json", bc).as_str()).expect("Culdn't find JSON map asset").path();
			let dat = dvec[0].to_owned();
			vec!((dat, json.to_owned()))
		} else {
			let mut v = Vec::new();
			for dat in dvec.iter() {
				let json = gem_bs.get_asset(format!("{}_map.json", dat).as_str()).expect("Culdn't find JSON map asset").path();
				v.push((dat.to_owned(), json.to_owned()))
			}
			v
		};
		json_files.push(SampleJsonFiles{barcode: bc.to_owned(), json_files: v});
	}

	let com = QPipeCom::MapReport((project, json_files));
	pipeline.add_com(com);
	pipeline		
}

pub fn make_merge_call_jsons_pipeline(gem_bs: &GemBS, job: usize) -> QPipe
{
	let task = &gem_bs.get_tasks()[job];
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	for out in task.outputs() { pipeline.add_outputs(gem_bs.get_asset(*out).expect("Couldn't get merge-call-jsons output asset").path()); }
	let bc = task.barcode().expect("No barcode set for merge-call-jsons task");
	let json_files: Vec<_> = task.inputs().map(|x| {
		let asset = gem_bs.get_asset(*x).expect("Couldn't find JSON file asset");
		pipeline.add_remove_file(asset.path());
		(asset.id().to_owned(), asset.path().to_owned())
	}).collect();
	let com = QPipeCom::MergeCallJsons(SampleJsonFiles{barcode: bc.to_owned(), json_files});
	pipeline.add_com(com);
	pipeline		
}

pub fn merge_call_jsons(sig: Arc<AtomicUsize>, outputs: &[PathBuf], sfiles: &SampleJsonFiles) -> Result<Option<Box<dyn BufRead>>, String> {
	let mut combined_stats: Option<CallJson> = None;
	for (_, path) in sfiles.json_files.iter() {
		utils::check_signal(Arc::clone(&sig))?;
		let rdr = compress::open_bufreader(path).map_err(|e| format!("{}", e))?;
		let jstats = CallJson::from_reader(rdr)?;
		combined_stats = if let Some(mut st) = combined_stats { st.merge(&jstats); Some(st) }
		else { Some(jstats) }
	}
	utils::check_signal(sig)?;
	if let Some(st) = combined_stats {
		let output = outputs.first().expect("No output file for merge JSON command");
		let wrt = compress::open_bufwriter(&output).map_err(|e| format!("{}", e))?;
		st.to_writer(wrt)?;
		Ok(None)
	} else { Err("OK".to_string()) }
}

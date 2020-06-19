use std::collections::HashMap;
use regex::{Regex, RegexSet};
use lazy_static::lazy_static;

use crate::config::GemBS;
use crate::common::assets::{Asset, GetAsset};
use crate::common::defs::{DataValue, Section, Metadata, FileType};
use crate::common::tasks::Task;
use super::QPipe;

fn check_inputs<'a>(gem_bs: &'a GemBS, task: &'a Task) -> (Vec<&'a Asset>, &'a str) {
	lazy_static! {
        static ref REFILE: Regex = Regex::new(r"^(.*)_read([12]?)$").unwrap();
        static ref REINDEX: Regex = Regex::new(r"^(.*_)?index$").unwrap();
    }
	let mut files = [None, None, None, None];
	let mut dataset = None;
	for ix in task.inputs() {
		let asset = gem_bs.get_asset(*ix).expect("Missing asset");
		if let Some(cap) = REFILE.captures(asset.id()) {
			let x = match cap.get(2) {
				Some(i) => { if i.as_str() == "1" { 1 } else { 2 }},
				None => 3,
			};
			files[x] = Some(ix);
			if let Some(dat) = cap.get(1) { dataset = Some(dat.as_str()); }
		} else if REINDEX.is_match(asset.id()) {
			files[0] = Some(ix);
		}
	}
	let dataset = dataset.expect("No dataset found for task");
	let index = gem_bs.get_asset(*files[0].expect("No index file found for task")).unwrap();
	let mut vfile = Vec::new();
	if let Some(f) = files[3] { vfile.push(gem_bs.get_asset(*f).unwrap()) }
	else {
		if let Some(f) = files[1] { vfile.push(gem_bs.get_asset(*f).unwrap()) }	
		if let Some(f) = files[2] { vfile.push(gem_bs.get_asset(*f).unwrap()) }
	}
	if vfile.is_empty() { panic!("No datafiles found for task") };
	vfile.push(index);
	(vfile, dataset)	
}

fn check_outputs<'a>(gem_bs: &'a GemBS, task: &'a Task) -> [Option<&'a Asset>; 3] {
	lazy_static! {
        static ref RE: RegexSet = RegexSet::new(&[r"^.*\.bam$", r"^.*\.cram$", r"^.*\.json$"]).unwrap();
	}
	let mut ofiles = [None, None, None];
	for ix in task.outputs() {
		let asset = gem_bs.get_asset(*ix).expect("Missing asset");
		RE.matches(asset.id()).into_iter().for_each(|x| ofiles[x] = Some(asset));
	}
	if ofiles[2].is_none() || (ofiles[0].is_none() && ofiles[1].is_none()) { panic!("Missing output files!"); }
	ofiles
}


fn get_read_groups(gem_bs: &GemBS, dataset: &str, href: &HashMap<Metadata, DataValue>) -> String {
	let sample = if let Some(DataValue::String(x)) = href.get(&Metadata::SampleName) { x } else { "" };
	let barcode = if let Some(DataValue::String(x)) = href.get(&Metadata::SampleBarcode) { x } else { "" };
	let mut read_groups = format!("@RG\\tID:{}\\tSM:{}\\tBC:{}\\tPU:{}", dataset, sample, barcode, dataset);
	for (tp, nm) in &[(Metadata::Description, "DS"), (Metadata::LibraryBarcode, "LB"), (Metadata::Centre, "CN"), (Metadata::Platform, "PL")] {
		if let Some(DataValue::String(x)) = href.get(tp) { read_groups.push_str(format!("\\t{}:{}", nm, x).as_str()) }}
	read_groups
}

pub fn make_map_pipeline(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>, job: usize) -> QPipe
{
	let threads = gem_bs.get_config_int(Section::Mapping, "threads");
	let mapping_threads = gem_bs.get_config_int(Section::Mapping, "mapping_threads").or(threads);
	let mut pipeline = QPipe::new();
	let mapper_path = gem_bs.get_exec_path("gem-mapper");
	let mut mapper_args = if let Some(t) = mapping_threads { format!("--threads {} ", t) } else { String::new() };
	let task = &gem_bs.get_tasks()[job];
	
	// Check inputs
	let (mut vfile, dataset) = check_inputs(gem_bs, task);
	let index = vfile.pop().expect("No index found!");
	
	// Check outputs
	let outs = check_outputs(gem_bs, task);
	
	let href = gem_bs.get_sample_data_ref().get(dataset).unwrap_or_else(|| panic!("No sample data for dataset {}", dataset));
	// Set read_groups
	let read_groups = get_read_groups(gem_bs, dataset, href);

	let ftype = if let Some(DataValue::FileType(t)) = href.get(&Metadata::FileType) { Some(*t) } else { None };
	let paired = if let Some(DataValue::Bool(x)) = options.get("paired") { *x } else {
		match ftype {
			Some(FileType::Paired) | Some(FileType::Interleaved) => true,
			_ => false,
		}
	};
	mapper_args.push_str(format!("-I {} ", index.path().to_string_lossy()).as_str());
	if vfile.len() == 2 {
		mapper_args.push_str(format!("--i1 {} --i2 {} ", vfile[0].path().to_string_lossy(), vfile[1].path().to_string_lossy()).as_str());
	} else if let Some(FileType::BAM) = ftype {
		let bam2fq = gem_bs.get_exec_path("samtools");
		let args = if let Some(t) = mapping_threads { format!("bam2fq {} --threads {}", vfile[0].path().to_string_lossy(), t) } 
		else { format!("bam2fq {}", vfile[0].path().to_string_lossy()) };
		pipeline.add_stage(&bam2fq, &args);
	} else { mapper_args.push_str(format!("-i {} ", vfile[0].path().to_string_lossy()).as_str()) }
	if gem_bs.get_config_bool(Section::Mapping, "benchmark_mode") { mapper_args.push_str("--benchmark-mode ") }
	if paired { mapper_args.push_str("--paired-end-alignment ")}
	if gem_bs.get_config_bool(Section::Mapping, "non_stranded") { mapper_args.push_str("--bisulfite-conversion non-stranded ") }
	else if gem_bs.get_config_bool(Section::Mapping, "reverse_conversion") { mapper_args.push_str("--bisulfite-conversion inferred-G2A-C2T ") }
	else { mapper_args.push_str("--bisulfite-conversion inferred-C2T-G2A ") }
	if let Some(x) = gem_bs.get_config_str(Section::Mapping, "underconversion_sequence") {
		mapper_args.push_str(format!("--underconversion-sequence {} ", x).as_str())	
	}
	if let Some(x) = gem_bs.get_config_str(Section::Mapping, "overconversion_sequence") {
		mapper_args.push_str(format!("--overconversion-sequence {} ", x).as_str())	
	}
	mapper_args.push_str(format!("--report-file {} ", outs[2].unwrap().path().to_string_lossy()).as_str());
	mapper_args.push_str(format!("--sam-read-group-header {}", read_groups).as_str());
	pipeline.add_stage(&mapper_path, &mapper_args);
	pipeline
}

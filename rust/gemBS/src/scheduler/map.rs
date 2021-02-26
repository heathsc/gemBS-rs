use std::collections::HashMap;
use std::path::Path;

use regex::Regex;
use lazy_static::lazy_static;

use crate::config::GemBS;
use crate::common::assets::{Asset, GetAsset};
use crate::common::defs::{DataValue, Section, Metadata, FileType, VarType};
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
       static ref REBAM: Regex = Regex::new(r"^.*\.(bam|cram)$").unwrap();
       static ref REJSON: Regex = Regex::new(r"^.*\.json$").unwrap();
	}
	let mut ofiles = [None, None, None];
	for ix in task.outputs() {
		let asset = gem_bs.get_asset(*ix).expect("Missing asset");
		if let Some(cap) = REBAM.captures(asset.id()) {
			let x = match cap.get(1) {
				Some(i) => { if i.as_str() == "bam" { 0 } else { 1 }},
				None => panic!("Unexpected match"),
			};
			ofiles[x] = Some(asset);
		} else if REJSON.is_match(asset.id()) {
			ofiles[2] = Some(asset);
		}
	}
	if ofiles[2].is_none() || (ofiles[0].is_none() && ofiles[1].is_none()) { panic!("Missing output files!"); }
	ofiles
}


fn get_read_groups(dataset: &str, href: &HashMap<Metadata, DataValue>) -> String {
	let sample = if let Some(DataValue::String(x)) = href.get(&Metadata::SampleName) { x } else { "" };
	let barcode = if let Some(DataValue::String(x)) = href.get(&Metadata::SampleBarcode) { x } else { "" };
	let mut read_groups = format!("@RG\\tID:{}\\tSM:{}\\tBC:{}\\tPU:{}", dataset, sample, barcode, dataset);
	for (tp, nm) in &[(Metadata::Description, "DS"), (Metadata::LibraryBarcode, "LB"), (Metadata::Centre, "CN"), (Metadata::Platform, "PL")] {
		if let Some(DataValue::String(x)) = href.get(tp) { read_groups.push_str(format!("\\t{}:{}", nm, x).as_str()) }}
	read_groups
}

pub fn make_map_pipeline(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>, job: usize) -> QPipe
{
	lazy_static! {
    	static ref OPT_LIST: Vec<(&'static str, &'static str, VarType)> = {
        	let mut m = Vec::new();
        	m.push(("underconversion_sequence", "underconversion-sequence", VarType::String));
        	m.push(("overconversion_sequence", "overconversion-sequence", VarType::String));
        	m.push(("benchmark_mode", "benchmark-mode", VarType::Bool));
			m
		};
	}
	let threads = gem_bs.get_config_int(Section::Mapping, "threads");
	let mapping_threads = gem_bs.get_config_int(Section::Mapping, "mapping_threads").or(threads);
	let sort_threads = gem_bs.get_config_int(Section::Mapping, "sort_threads").or(mapping_threads);
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	let mapper_path = gem_bs.get_exec_path("gem-mapper");
	let mut mapper_args = if let Some(t) = mapping_threads { format!("--threads\x1e{}\x1e", t) } else { String::new() };
	let task = &gem_bs.get_tasks()[job];
	if let Some(x) = task.log() { pipeline.log = Some(gem_bs.get_asset(x).expect("Couldn't get log file").path().to_owned()) }
	// Check type of mapping
	let single_bam = task.id().starts_with("single_map");
		
	// Check inputs
	let (mut vfile, dataset) = check_inputs(gem_bs, task);
	let index = vfile.pop().expect("No index found!");
	
	// Check outputs
	let outs = check_outputs(gem_bs, task);
	let (outfile, cram) = if let Some(x) = outs[0] { (x, false) } else if let Some(x) = outs[1] { (x, true) } else { panic!("No mapping outfile set!") };
	let tmp_dir = match gem_bs.get_config_str(Section::Mapping, "tmp_dir") {
		Some(x) => Some(Path::new(x)),
		None => outfile.path().parent(),
	};
		
	let href = gem_bs.get_sample_data_ref().get(dataset).unwrap_or_else(|| panic!("No sample data for dataset {}", dataset));
	// Set read_groups
	let read_groups = get_read_groups(dataset, href);

	// Setup rest of arguments for mapper
	let ftype = if let Some(DataValue::FileType(t)) = href.get(&Metadata::FileType) { Some(*t) } else { None };
	let paired = if let Some(DataValue::Bool(x)) = options.get("paired") { *x } else { matches!(ftype, Some(FileType::Paired) | Some(FileType::Interleaved)) };

	mapper_args.push_str(format!("-I\x1e{}\x1e", index.path().display()).as_str());
	if vfile.len() == 2 {
		mapper_args.push_str(format!("--i1\x1e{}\x1e--i2\x1e{}\x1e", vfile[0].path().display(), vfile[1].path().display()).as_str());
	} else if let Some(FileType::BAM) = ftype {
		let bam2fq = gem_bs.get_exec_path("samtools");
		let args = if let Some(t) = mapping_threads { format!("bam2fq\x1e{}\x1e--threads\x1e{}", vfile[0].path().display(), t) } 
		else { format!("bam2fq {}", vfile[0].path().display()) };
		pipeline.add_stage(&bam2fq, &args);
	} else { mapper_args.push_str(format!("-i\x1e{}\x1e", vfile[0].path().display()).as_str()) }
	if paired { mapper_args.push_str("--paired-end-alignment\x1e")}
	if gem_bs.get_config_bool(Section::Mapping, "non_stranded") { mapper_args.push_str("--bisulfite-conversion non-stranded\x1e") }
	else if gem_bs.get_config_bool(Section::Mapping, "reverse_conversion") { mapper_args.push_str("--bisulfite-conversion\x1einferred-G2A-C2T\x1e") }
	else { mapper_args.push_str("--bisulfite-conversion\x1einferred-C2T-G2A\x1e") }
	
	super::add_command_opts(gem_bs, &mut mapper_args, Section::Mapping, &OPT_LIST);

	mapper_args.push_str(format!("--report-file\x1e{}\x1e", outs[2].unwrap().path().display()).as_str());
	mapper_args.push_str(format!("--sam-read-group-header\x1e{}", read_groups).as_str());
	
	// Setup read_filter stage
	let read_filter = gem_bs.get_exec_path("read_filter");
	let contig_md5 = gem_bs.get_asset("contig_md5").expect("Couldn't find contig md5 asset");
	let read_filter_args = format!("{}", contig_md5.path().display());
	
	// Setup samtools stage
	let samtools = gem_bs.get_exec_path("samtools");
	let mut samtools_args = format!("sort\x1e-o\x1e{}\x1e", outfile.path().display());
	if let Some(x) = tmp_dir { samtools_args.push_str(format!("-T\x1e{}\x1e", x.display()).as_str())}
	if let Some(x) = gem_bs.get_config_str(Section::Mapping, "sort_memory") { samtools_args.push_str(format!("-m\x1e{}\x1e", x).as_str())}
	if let Some(x) = sort_threads { samtools_args.push_str(format!("--threads\x1e{}\x1e", x).as_str())}
	if single_bam { samtools_args.push_str("--write-index\x1e") }
	if cram { samtools_args.push_str("-O\x1eCRAM\x1e") }
	if gem_bs.get_config_bool(Section::Mapping, "benchmark_mode") { samtools_args.push_str("--no-PG\x1e") } else if cram {
		let gembs_ref = gem_bs.get_asset("gembs_reference").expect("Couldn't find gemBS reference asset");
		samtools_args.push_str(format!("--reference\x1e{}\x1e", gembs_ref.path().display()).as_str());
	}
	samtools_args.push('-');
	if gem_bs.get_config_bool(Section::Mapping, "keep_logs") { pipeline.set_remove_log(false) }
	for out in task.outputs() { pipeline.add_outputs(gem_bs.get_asset(*out).expect("Couldn't get md5sum output asset").path()); }

	pipeline.add_stage(&mapper_path, &mapper_args)
			.add_stage(&read_filter, &read_filter_args)
			.add_stage(&samtools, &samtools_args);
	pipeline
}

pub fn make_merge_bams_pipeline(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>, job: usize) -> QPipe
{
	let threads = gem_bs.get_config_int(Section::Mapping, "threads");
	let merge_threads = gem_bs.get_config_int(Section::Mapping, "merge_threads").or(threads);
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	let samtools_path = gem_bs.get_exec_path("samtools");
	let mut args = String::from("merge\x1e--write-index\x1e");
	if let Some(x) = merge_threads { args.push_str(format!("--threads\x1e{}\x1e", x).as_str())}
	let task = &gem_bs.get_tasks()[job];
	let output = gem_bs.get_asset(*task.outputs().next().expect("No output files for merge step")).expect("Couldn't get asset");
	let cram = output.id().ends_with(".cram");
	if cram { args.push_str("-O\x1ecram\x1e") }
	if gem_bs.get_config_bool(Section::Mapping, "benchmark_mode") { args.push_str("--no-PG\x1e") } else if cram {
		let gembs_ref = gem_bs.get_asset("gembs_reference").expect("Couldn't find gemBS reference asset");
		args.push_str(format!("--reference\x1e{}\x1e", gembs_ref.path().display()).as_str());
	}
	args.push_str(format!("-f\x1e{}\x1e", output.path().display()).as_str());
	let remove_bams = if let Some(DataValue::Bool(x)) = options.get("remove") { *x } else { 
		gem_bs.get_config_bool(Section::Mapping, "remove_individual_bams") };	
	for asset in task.inputs().map(|x| gem_bs.get_asset(*x).expect("Couldn't get asset")).filter(|x| x.id().ends_with(".bam")) {
		args.push_str(format!("{}\x1e", asset.path().display()).as_str());
		if remove_bams { pipeline.add_remove_file(&asset.path()); }
	}	
	if let Some(x) = task.log() { pipeline.log = Some(gem_bs.get_asset(x).expect("Couldn't get log file").path().to_owned()) }
	for out in task.outputs() { pipeline.add_outputs(gem_bs.get_asset(*out).expect("Couldn't get md5sum output asset").path()); }
	if gem_bs.get_config_bool(Section::Mapping, "keep_logs") { pipeline.set_remove_log(false) }
	pipeline.add_stage(&samtools_path, &args);
	pipeline
}

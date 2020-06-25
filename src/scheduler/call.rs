use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{BufWriter, Write};
use regex::Regex;
use lazy_static::lazy_static;

use crate::config::GemBS;
use crate::common::assets::{Asset, GetAsset};
use crate::common::defs::{DataValue, Section, Metadata, FileType, ContigInfo, ContigData, VarType};
use crate::common::tasks::Task;
use super::QPipe;

fn check_inputs<'a>(gem_bs: &'a GemBS, task: &'a Task) -> (usize, &'a str) {
	lazy_static! { static ref RE: Regex = Regex::new(r"^(.+)\.(bam|cram)").unwrap(); }
	let (mut in_bam, mut bc) = (None, None);
	for ix in task.inputs() {
		let asset = gem_bs.get_asset(*ix).expect("Missing asset");
		if let Some(cap) = RE.captures(asset.id()) {
			if let Some(x) = cap.get(1) {
				bc = Some(x.as_str());
				in_bam = Some(*ix);
				break;
			}
		}
	}
	(in_bam.expect("No input BAM file found for call"), bc.expect("No barcode found for call"))
}

// Parse pool from task argument if prsent
fn get_pool(task: &'_ Task) -> Option<&'_ str> {
	lazy_static! { static ref RE: Regex = Regex::new(r"--pool (\S+)$").unwrap(); }
	RE.captures(task.args()).map(|c| { c.get(1).expect("No match!").as_str() })	
}

fn make_pool_file(gem_bs: &GemBS, barcode: &str, pool: Option<&str>, output: &Path) -> Option<PathBuf> {
	if let Some(p) = pool {
		let parent = output.parent().unwrap_or_else(|| Path::new("."));
		let ctg_path: PathBuf = [parent, Path::new(format!("{}_{}_ctgs.bed", barcode, p).as_str())].iter().collect();
		let hr_ctg = gem_bs.get_contig_hash().get(&ContigInfo::Contigs).expect("No Contig defs entry");
		let hr_pool = gem_bs.get_contig_hash().get(&ContigInfo::ContigPools).expect("No Contig pools entry");
		let cpool = if let ContigData::ContigPool(x) = hr_pool.get(&p.to_owned()).expect("Unknown pool") {x} else {panic!("Wrong datatype")};
		let mut wr = BufWriter::new(fs::File::create(&ctg_path)
			.unwrap_or_else(|e| panic!("Couldn't open contig_sizes file {} for output: {}", ctg_path.to_string_lossy(), e)));
		for ctg_name in cpool.contigs.iter() {
			let ctg = if let ContigData::Contig(x) = hr_ctg.get(ctg_name).expect("No contig entry") {x} else {panic!("Wrong datatype")};
			writeln!(wr, "{}\t0\t{}", ctg.name, ctg.len)
				.unwrap_or_else(|e| panic!("Error writing to file {}: {}", ctg_path.to_string_lossy(), e));
		}
		Some(ctg_path)
	} else { None }
}

pub fn make_call_pipeline(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>, job: usize) -> QPipe
{
	lazy_static! {
    	static ref OPT_LIST: Vec<(&'static str, &'static str, VarType)> = {
        	let mut m = Vec::new();
        	m.push(("left_trim", "left-trim", VarType::Int));
        	m.push(("right_trim", "right-trim", VarType::Int));
        	m.push(("keep_unmatched", "keep-unmatched", VarType::Bool));
        	m.push(("keep_duplicates", "keep-duplicates", VarType::Bool));
        	m.push(("ignore_duplicate_flag", "ignore-duplicates", VarType::Bool));
        	m.push(("benchmark_mode", "benchmark-mode", VarType::Bool));
	       	m.push(("haploid", "haploid", VarType::Bool));
	       	m.push(("reference_bias", "reference-bias", VarType::Float));
	       	m.push(("mapq_threshold", "mapq-threshold", VarType::Int));
	       	m.push(("qual_threshold", "bq-threshold", VarType::Int));
			m
		};
	}
	let threads = gem_bs.get_config_int(Section::Calling, "threads");
	let call_threads = gem_bs.get_config_int(Section::Calling, "call_threads").or(threads);
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	let bs_call_path = gem_bs.get_exec_path("bs_call");
	let task = &gem_bs.get_tasks()[job];
	let (in_bam, barcode) = check_inputs(gem_bs, task);
	let pool = get_pool(task);
	let mut out_iter = task.outputs(); 
	let output_bcf = gem_bs.get_asset(*out_iter.next().expect("No output files for call step")).expect("Couldn't get asset").path();
	let report_file = gem_bs.get_asset(*out_iter.next().expect("No JSON file for call step")).expect("Couldn't get asset").path();
	let contig_pool = make_pool_file(gem_bs, barcode, pool, output_bcf);
	let contig_sizes = gem_bs.get_asset("contig_sizes").expect("Couldn't find contig sizes asset").path();
	let gembs_ref = gem_bs.get_asset("gembs_reference").expect("Couldn't find gemBS reference asset");
	
	// Set up bs_call arguments
	let mut args = format!("--output {} --output-type b --reference {} --sample {} --contig-sizes {} --report-file {} "
		, output_bcf.to_string_lossy(), gembs_ref.path().to_string_lossy(), barcode, contig_sizes.to_string_lossy(), report_file.to_string_lossy());
	if let Some(cp) = contig_pool { 
		args.push_str(format!("--contig-bed {} ", cp.to_string_lossy()).as_str());
		pipeline.add_remove_file(&cp);
	}
	if let Some(t) = call_threads { args.push_str(format!("--threads {} ", t).as_str()); }
	super::add_command_opts(gem_bs, &mut args, Section::Calling, &OPT_LIST);
	args.push_str(&gem_bs.get_asset(in_bam).unwrap().path().to_string_lossy());

	if let Some(x) = task.log() { pipeline.log = Some(gem_bs.get_asset(x).expect("Couldn't get log file").path().to_owned()) }
	for out in task.outputs() { pipeline.add_outputs(gem_bs.get_asset(*out).expect("Couldn't get md5sum output asset").path()); }
	if gem_bs.get_config_bool(Section::Calling, "keep_logs") { pipeline.set_remove_log(false) }
	pipeline.add_stage(&bs_call_path, &args);
	pipeline
}

pub fn make_merge_bcfs_pipeline(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>, job: usize) -> QPipe
{
	let threads = gem_bs.get_config_int(Section::Calling, "threads");
	let merge_threads = gem_bs.get_config_int(Section::Calling, "merge_threads").or(threads);
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	let bcftools_path = gem_bs.get_exec_path("bcftools");
	let task = &gem_bs.get_tasks()[job];
	let output_bcf = gem_bs.get_asset(*task.outputs().next().expect("No output files for call step")).expect("Couldn't get asset").path();

	// Setup arguments	
	let mut args = format!("concat --output {} --output-type b --naive ", output_bcf.to_string_lossy());
	if gem_bs.get_config_bool(Section::Calling, "benchmark_mode") { args.push_str("--no-version ")}		
	if let Some(t) = merge_threads { args.push_str(format!("--threads {} ", t).as_str()); }

	let remove_bcfs = if let Some(DataValue::Bool(x)) = options.get("remove") { *x } else { 
	gem_bs.get_config_bool(Section::Calling, "remove_individual_bcfs") };	
	let mut v = Vec::new();
	for asset in task.inputs().map(|x| gem_bs.get_asset(*x).expect("Couldn't get asset")).filter(|x| x.id().ends_with(".bcf")) {
		let s = asset.path().to_string_lossy();
		v.push(s);
		if remove_bcfs { pipeline.add_remove_file(&asset.path()); }
	}
	v.sort();
	for s in v.iter() { args.push_str(format!("{} ", s).as_str()) }
	if let Some(x) = task.log() { pipeline.log = Some(gem_bs.get_asset(x).expect("Couldn't get log file").path().to_owned()) }
	for out in task.outputs() { pipeline.add_outputs(gem_bs.get_asset(*out).expect("Couldn't get md5sum output asset").path()); }
	if gem_bs.get_config_bool(Section::Mapping, "keep_logs") { pipeline.set_remove_log(false) }

	pipeline.add_stage(&bcftools_path, &args);
	pipeline
}

pub fn make_index_bcf_pipeline(gem_bs: &GemBS, job: usize) -> QPipe
{
	let threads = gem_bs.get_config_int(Section::Calling, "threads");
	let merge_threads = gem_bs.get_config_int(Section::Calling, "merge_threads").or(threads);
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	let bcftools_path = gem_bs.get_exec_path("bcftools");
	let task = &gem_bs.get_tasks()[job];
	let input = gem_bs.get_asset(*task.inputs().next().expect("No input file for index bcf step")).expect("Couldn't get asset").path();

	// Setup arguments	
	let mut args = String::from("index ");
	if let Some(t) = merge_threads { args.push_str(format!("--threads {} ", t).as_str()); }
	args.push_str(format!("{}", input.to_string_lossy()).as_str());
	for out in task.outputs() { pipeline.add_outputs(gem_bs.get_asset(*out).expect("Couldn't get md5sum output asset").path()); }

	pipeline.add_stage(&bcftools_path, &args);
	pipeline
}
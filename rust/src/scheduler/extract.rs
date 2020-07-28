use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::fs;
use std::rc::Rc;
use std::io::{BufWriter, Write};
use regex::Regex;
use lazy_static::lazy_static;

use crate::config::GemBS;
use crate::common::assets::GetAsset;
use crate::common::defs::{Section, ContigInfo, ContigData, VarType};
use super::QPipe;

fn make_contig_file(gem_bs: &GemBS, barcode: &str, output_dir: &Path) -> PathBuf {
	let ctg_path: PathBuf = [output_dir, Path::new(format!("{}_mextr_ctgs.bed", barcode).as_str())].iter().collect();
	let hr_ctg = gem_bs.get_contig_hash().get(&ContigInfo::Contigs).expect("No Contig defs entry");
	let omit_hash = {
		if let Some(oc) = gem_bs.get_config_stringvec(Section::Index, "omit_ctgs") {
			oc.iter().fold(HashSet::new(), |mut h, x| { h.insert(x.clone()); h })	
		} else { HashSet::new() }
	};
	let mut ctg_list: Vec<_> = hr_ctg.keys().filter(|x| !omit_hash.contains((*x).as_str())).map(|x| Rc::clone(x)).collect();
	ctg_list.sort();
	let mut wr = BufWriter::new(fs::File::create(&ctg_path)
		.unwrap_or_else(|e| panic!("Couldn't open contig_sizes file {} for output: {}", ctg_path.to_string_lossy(), e)));
	for ctg_name in ctg_list.iter() {
		let ctg = if let ContigData::Contig(x) = hr_ctg.get(ctg_name).expect("No contig entry") {x} else {panic!("Wrong datatype")};
		writeln!(wr, "{}\t0\t{}", ctg.name, ctg.len)
			.unwrap_or_else(|e| panic!("Error writing to file {}: {}", ctg_path.to_string_lossy(), e))
	}
	ctg_path
}

fn make_mextr_pipeline(gem_bs: &GemBS, job: usize, bc: &str) -> QPipe {
	let task = &gem_bs.get_tasks()[job];
	let first_output = gem_bs.get_asset(*task.outputs().next().expect("No output files for extract step")).expect("Couldn't get asset").path();
	let in_bcf = gem_bs.get_asset(*task.inputs().next().expect("No output files for extract step")).expect("Couldn't get asset").path();
	let output_dir = first_output.parent().unwrap_or_else(|| Path::new("."));
	let contig_file = make_contig_file(gem_bs, bc, output_dir);
	let mextr_path = gem_bs.get_exec_path("mextr");
	
	// Set up arg list
	let mut args = format!("--bgzip --md5 --regions-file {} ", contig_file.to_string_lossy());
	let (mut cpg, mut noncpg, mut bedmethyl) = (false, false, false);
	for out in task.outputs() {
		let oname = gem_bs.get_asset(*out).expect("Couldn't get output asset").path().to_string_lossy();
		if oname.ends_with("non_cpg.txt.gz") { 
			noncpg = true;
			args.push_str(format!("--noncpgfile {} ", oname).as_str())
		} else if oname.ends_with("cpg.txt.gz") {
			cpg = true; 
			args.push_str(format!("--cpgfile {} ", oname).as_str())
		} else if oname.ends_with("cpg.bed.gz") { 
			bedmethyl = true;
			let outbase: PathBuf = [output_dir, Path::new(bc)].iter().collect();	
			args.push_str(format!("--bed-methyl {} ", outbase.to_string_lossy()).as_str())
		}
	}
	let mut opt_list = Vec::new();
	opt_list.push(("threads", "threads", VarType::Bool));
   	opt_list.push(("reference_bias", "reference-bias", VarType::Float));
    	opt_list.push(("qual_threshold", "bq-threshold", VarType::Int));
	if cpg || noncpg { 
		args.push_str("--tabix ");
	  	opt_list.push(("phred_threshold", "threshold", VarType::Int));
		opt_list.push(("min_inform", "inform", VarType::Int));
		opt_list.push(("allow_het", "select het", VarType::Bool));
	}
	if noncpg { opt_list.push(("min_nc", "min-nc", VarType::Int)); }
	if cpg { opt_list.push(("strand_specific", "mode strand-specific", VarType::Bool)); }
	if bedmethyl { opt_list.push(("bigwig_strand_specific", "bw-mode strand-specific", VarType::Bool)); }
	super::add_command_opts(gem_bs, &mut args, Section::Extract, &opt_list);
	args.push_str(&in_bcf.to_string_lossy());

	// Setup mextr pipeline
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	if let Some(x) = task.log() { pipeline.log = Some(gem_bs.get_asset(x).expect("Couldn't get log file").path().to_owned()) }
	for out in task.outputs() { pipeline.add_outputs(gem_bs.get_asset(*out).expect("Couldn't get mextr output asset").path()); }
	if gem_bs.get_config_bool(Section::Extract, "keep_logs") { pipeline.set_remove_log(false) }
	pipeline.add_stage(&mextr_path, &args).add_remove_file(&contig_file);
	pipeline	
}

fn make_snpxtr_pipeline(gem_bs: &GemBS, job: usize) -> QPipe {
	let task = &gem_bs.get_tasks()[job];
	let first_out = gem_bs.get_asset(*task.outputs().next().expect("No output files for extract step")).expect("Couldn't get asset").path();
	let in_bcf = gem_bs.get_asset(*task.inputs().next().expect("No output files for extract step")).expect("Couldn't get asset").path();
	let snpxtr_path = gem_bs.get_exec_path("snpxtr");

	// Set up arg list
	let mut args = format!("--bgzip --md5 --tabix --output {} ", first_out.to_string_lossy());
	let mut opt_list = Vec::new();
	opt_list.push(("threads", "threads", VarType::Bool));
	opt_list.push(("snp_list", "snps", VarType::String));
	opt_list.push(("dbsnp_index", "dbsnp", VarType::String));
	super::add_command_opts(gem_bs, &mut args, Section::Extract, &opt_list);
	args.push_str(&in_bcf.to_string_lossy());

	// Setup snpxtr pipeline
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	if let Some(x) = task.log() { pipeline.log = Some(gem_bs.get_asset(x).expect("Couldn't get log file").path().to_owned()) }
	for out in task.outputs() { pipeline.add_outputs(gem_bs.get_asset(*out).expect("Couldn't get snpxtr output asset").path()); }
	if gem_bs.get_config_bool(Section::Extract, "keep_logs") { pipeline.set_remove_log(false) }
	pipeline.add_stage(&snpxtr_path, &args);
	pipeline	
}

fn get_command_and_barcode(id: &str) -> (&str, &str) {
	lazy_static! { static ref RE: Regex = Regex::new(r"^(mextr|snpxtr)_(.*)$").unwrap(); }
	if let Some(cap) = RE.captures(id) {
		(cap.get(1).unwrap().as_str(), cap.get(2).unwrap().as_str())	
	} else { panic!("Couldn't parse extract task id") }
}

pub fn make_extract_pipeline(gem_bs: &GemBS, job: usize) -> QPipe
{
	match get_command_and_barcode(gem_bs.get_tasks()[job].id()) {
		("mextr", bc) => make_mextr_pipeline(gem_bs, job, bc),
		("snpxtr", _) => make_snpxtr_pipeline(gem_bs, job),
		_ => panic!("Couldn't parse extract task id"),
	}
}



// Check requirements and presence of reference, index files and index_dir
// Make gemBS reference if required
// Make asset list for refererences, indicies and other associated files

use crate::common::defs::{Section, Metadata, DataValue, Command, ContigInfo, ContigData};
use crate::config::GemBS;
use crate::common::utils::Pipeline;
use crate::common::{assets, compress};
use crate::common::assets::{AssetType, GetAsset};

use std::str::FromStr;
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{BufRead, BufWriter, Write};
use glob::glob;
use std::collections::HashSet;

fn check_ref(gem_bs: &mut GemBS) -> Result<(), String> {
	// Check reference file
	let reference = gem_bs.get_reference()?;
	let tpath = PathBuf::from(reference);
	if !tpath.exists() { return Err(format!("Reference file {} does not exist or is not accessible", reference)); }
	debug!("Reference file {} found", reference);
	gem_bs.insert_asset("reference", &tpath, AssetType::Supplied);
	// Check extra references - these are not required, but if specified in the config file, the file should be present
	let extra_ref = gem_bs.get_config(Section::Index, "extra_references").cloned();
	if let Some(DataValue::String(ref_file)) = extra_ref {
		gem_bs.check_signal()?;
		let tpath = Path::new(&ref_file);
		if !tpath.exists() { return Err(format!("Extra references file {} does not exist or is not accessible", ref_file)); }
		debug!("Extra references file {} found", ref_file);
		gem_bs.insert_asset("extra_reference", tpath, AssetType::Supplied);
		trace!("Getting names of contigs in extra references file {}", ref_file);
		let rdr = compress::open_bufreader(tpath).map_err(|x| format!("{}", x))?;
		let mut omit_ctgs = Vec::new();
		for line in rdr.lines() {
			if let Ok(s) = line {
				if s.starts_with('>') { omit_ctgs.push(s.trim_start_matches('>').to_string()) }
			}
		}
		if !omit_ctgs.is_empty() { gem_bs.set_config(Section::Index, "omit_ctgs", DataValue::StringVec(omit_ctgs)); }
	} 	
	gem_bs.check_signal()?;
	Ok(())
}

fn check_index_requirements(gem_bs: &GemBS) -> (bool, bool) {
	// Check if we need  regular (nonbs) index and bs index
	let mut need_bs_index = false;
	let mut need_nonbs_index = false;
	let tref = gem_bs.get_sample_data_ref();
	for (_, hr) in tref.iter() {
		if let Some(DataValue::Bool(b)) = hr.get(&Metadata::Bisulfite) {
			if *b { need_bs_index = true; } else { need_nonbs_index = true; }
		} else { need_bs_index = true;}	
	}
	(need_bs_index, need_nonbs_index)	
}
fn check_indices(gem_bs: &mut GemBS) -> Result<(), String> {	

	let reference = gem_bs.get_reference()?;
	let (need_bs_index, need_nonbs_index) = check_index_requirements(gem_bs);
	
	// Check index and indexdir.  One of these at least should exist and then the other can be inferred.
	let mut infer_idx = None;
	let mut infer_nonbs_idx = None;
	let mut infer_idx_dir = None;
	let mut missing_nonbs_index = false;
	let mut missing_index = false;
	let infer_parent = |s: String| {
		let tpath = Path::new(&s);
		let tp = if let Some(d) = tpath.parent() {
			if d.as_os_str().is_empty() { None }
			else { Some(d) }
		} else { None };
		if let Some(d) = tp { d.to_owned() } else { PathBuf::from_str(".").unwrap() }
	}; 
	if need_nonbs_index && gem_bs.get_config(Section::Index, "nonbs_index").is_none() { missing_nonbs_index = true; }
	if need_bs_index && gem_bs.get_config(Section::Index, "bs_index").is_none() { missing_index = true; }

	if missing_index || missing_nonbs_index {			
		// Check for index_dir
		let idx_dir = if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Index, "index_dir") { PathBuf::from(x) }
		else {
			if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Index, "bs_index")	{ infer_idx_dir = Some(infer_parent(x.clone())); }	
			else if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Index, "nonbs_index")	{ infer_idx_dir = Some(infer_parent(x.clone())); }	
			else { infer_idx_dir = Some(PathBuf::from_str(".").unwrap()) };
			infer_idx_dir.clone().unwrap()			
		};	
		// Check directory exists
		if !idx_dir.is_dir() { 
			if let Err(e) = fs::create_dir(&idx_dir) {
				return Err(format!("Could not create index_dir directory {}: {}", idx_dir.display(), e)); 
			}
		} 
		if missing_index {
			let tpath = Path::new(Path::new(reference).file_stem().unwrap()).with_extension("BS.gem");	
			let mut idx = idx_dir.clone();
			idx.push(tpath);		
			infer_idx = Some(idx.to_str().unwrap().to_string());
		}
		if missing_nonbs_index {
			let tpath = Path::new(Path::new(reference).file_stem().unwrap()).with_extension("gem");	
			let mut idx = idx_dir;
			idx.push(tpath);		
			infer_nonbs_idx = Some(idx.to_str().unwrap().to_string());
		}
		
	}
	if let Some(x) = infer_idx {
		gem_bs.set_config(Section::Index, "index", DataValue::String(x));
	}
	if let Some(x) = infer_nonbs_idx {
		gem_bs.set_config(Section::Index, "nonbs_index", DataValue::String(x));
	}
	if let Some(x) = infer_idx_dir {
		gem_bs.set_config(Section::Index, "index_dir", DataValue::String(format!("{}", x.display())));
	}
	gem_bs.set_config(Section::Index, "need_bs_index", DataValue::Bool(need_bs_index));
	gem_bs.set_config(Section::Index, "need_nonbs_index", DataValue::Bool(need_nonbs_index));
	if need_bs_index {
		if let Some(DataValue::String(index)) = gem_bs.get_config(Section::Index, "index").cloned() {
			gem_bs.insert_asset("index", Path::new(&index), AssetType::Derived);			
		} else { return Err("Internal error - no index".to_string()); }
	}
	if need_nonbs_index {
		if let Some(DataValue::String(index)) = gem_bs.get_config(Section::Index, "nonbs_index").cloned() {
			gem_bs.insert_asset("nonbs_index", Path::new(&index), AssetType::Derived);			
		} else { return Err("Internal error - no index".to_string()); }
	}
	gem_bs.check_signal()	
}


fn make_dbsnp_tasks(gem_bs: &mut GemBS, dbsnp_files: Vec<PathBuf>) {
	let dbsnp_index = if let Some(DataValue::String(idx)) = gem_bs.get_config(Section::Index, "dbsnp_index") { PathBuf::from(idx) } 
	else { 
		let dir = if let Some(DataValue::String(idx)) = gem_bs.get_config(Section::Index, "index_dir" ) { idx } else { "." };
		let p: PathBuf = [dir, "dbSNP_gemBS.idx"].iter().collect();
		gem_bs.set_config(Section::Index, "dbsnp_index", DataValue::String(p.to_string_lossy().to_string()));
		p
	};
	let mut in_vec = Vec::new();
	for (ix, f) in dbsnp_files.iter().enumerate() { in_vec.push(gem_bs.insert_asset(format!("dbsnp_file_{}", ix + 1).as_str(), &f, AssetType::Supplied)); }
	let index = gem_bs.insert_asset("dbsnp_index", &dbsnp_index, AssetType::Derived);
	let (id, desc, command, args) = ("dbsnp_index", "Generate dbSNP index", Command::Index, "--dbsnp-index");
	let (log_name, log_path) = assets::derive_log_asset(id, &dbsnp_index);
	let log_index = gem_bs.insert_asset(&log_name, &log_path, AssetType::Log);
	let index_task = gem_bs.add_task(id, desc, command, args);
	let cores = gem_bs.get_config_int(Section::Index, "cores").map(|x| x as usize);
	let memory = gem_bs.get_config_memsize(Section::Index, "memory");
	let time = gem_bs.get_config_joblen(Section::Index, "time").or_else(|| Some(21600.into()));
	gem_bs.add_task_inputs(index_task, &in_vec).add_outputs(&[index]).set_log(Some(log_index))
		.add_cores(cores).add_memory(memory).add_time(time);
	gem_bs.get_asset_mut(index).unwrap().set_creator(index_task, &in_vec);	
}

fn check_dbsnp_ref(gem_bs: &mut GemBS) -> Result<(), String> {	
	gem_bs.check_signal()?;
	if let Some(DataValue::StringVec(dbsnp_files)) = gem_bs.get_config(Section::Index, "dbsnp_files") { 
		let mut files = Vec::new();
		for pat in dbsnp_files.iter() {
			for mat in glob(pat).map_err(|e| format!("{}",e))? {
				match mat {
					Ok(f) => files.push(f),
					Err(e) => return Err(format!("{}", e)),
				}
			}
		}
		if !files.is_empty() { make_dbsnp_tasks(gem_bs, files); }
	}
	gem_bs.check_signal()
}

pub fn make_contig_sizes(gem_bs: &mut GemBS) -> Result<(), String> {	
	gem_bs.check_signal()?;
	let reference = gem_bs.get_reference()?;
	let tpath = Path::new(Path::new(reference).file_stem().unwrap()).with_extension("gemBS.contig_sizes");
	let index_dir = if let Some(DataValue::String(idx)) = gem_bs.get_config(Section::Index, "index_dir") { idx } else { panic!("Internal error - missing index_dir") }; 
	let index_dir = Path::new(index_dir);
	let contig_sizes: PathBuf = [index_dir, &tpath].iter().collect();
	if !contig_sizes.exists() {
		info!("Creating contig sizes file {}", contig_sizes.to_string_lossy());
		let omit_ctgs = if let Some(DataValue::StringVec(v)) = gem_bs.get_config(Section::Index, "omit_ctgs") {
			v.iter().fold(HashSet::new(), |mut h, x| { h.insert(x.as_str()); h })
		} else { HashSet::new() };
		let hr = gem_bs.get_contig_hash().get(&ContigInfo::Contigs).expect("No Contig defs entry");
		let mut wr = BufWriter::new(fs::File::create(&contig_sizes)
			.map_err(|e| format!("Couldn't open contig_sizes file {} for output: {}", contig_sizes.to_string_lossy(), e))?);
		gem_bs.check_signal()?;
		for (_, ctgdat) in hr.iter() {
			if let ContigData::Contig(ctg) = ctgdat {
				if !omit_ctgs.contains(ctg.name.as_str()) { writeln!(wr, "{}\t{}", ctg.name, ctg.len)
						.map_err(|e| format!("Error writing to file {}: {}", contig_sizes.to_string_lossy(), e))?;
				}
			}
		}			
	}
	gem_bs.insert_asset("contig_sizes", &contig_sizes, AssetType::Derived);	
	Ok(())
}
	
fn make_gem_ref(gem_bs: &mut GemBS) -> Result<(), String> {	
	let reference = gem_bs.get_reference()?;
	let index_dir = if let Some(DataValue::String(idx)) = gem_bs.get_config(Section::Index, "index_dir") { idx } else { panic!("Internal error - missing index_dir") }; 
	let tpath = Path::new(Path::new(reference).file_stem().unwrap()).with_extension("gemBS.ref");	
	let mut gref = PathBuf::from(index_dir);
	gref.push(tpath);
	let gref_fai = gref.with_extension("ref.fai");
	let gref_gzi = gref.with_extension("ref.gzi");
	let tpath = Path::new(Path::new(reference).file_stem().unwrap()).with_extension("gemBS.contig_md5");
	let mut ctg_md5 = PathBuf::from(index_dir);
	ctg_md5.push(tpath);
	// Create gemBS reference if it does not already exist		
	if !(gref.exists() && ctg_md5.exists()) {
		gem_bs.check_signal()?;
		info!("Creating gemBS compressed reference and calculating md5 sums of contigs");
		let _ = fs::remove_file(&gref_fai);
		let _ = fs::remove_file(&gref_gzi);
		let mut md5_args = vec!("-o", ctg_md5.to_str().unwrap(), "-s");
		let populate_cache = if let Some(DataValue::Bool(x)) = gem_bs.get_config(Section::Index, "populate_cache") { *x } else { false };
		if populate_cache { md5_args.push("-p"); }
		md5_args.push(reference);
		if let Some(DataValue::String(s)) = gem_bs.get_config(Section::Index, "extra_references") { md5_args.push(s); }
		let md5_path = gem_bs.get_exec_path("md5_fasta");
		let thr = gem_bs.get_threads(Section::Index).to_string();
		let bgzip_args = vec!("-@", &thr);
		let bgzip_path = gem_bs.get_exec_path("bgzip");
		let mut pipeline = Pipeline::new();
		pipeline.add_stage(&md5_path, Some(md5_args.iter()))
			    .add_stage(&bgzip_path, Some(bgzip_args.iter()))
				.out_file(&gref).add_output(&ctg_md5);
		pipeline.run(gem_bs.get_signal_clone())?;
	}
	// Create faidx index if required		
	if !(gref_fai.exists() && gref_gzi.exists()) {
		gem_bs.check_signal()?;
		info!("Creating gemBS faidx index");
		let faidx_args = vec!("faidx", gref.to_str().unwrap());
		let samtools_path = gem_bs.get_exec_path("samtools");
		let mut pipeline = Pipeline::new();
		pipeline.add_stage(&samtools_path, Some(faidx_args.iter()))
				.add_output(&gref_fai).add_output(&gref_gzi);
		pipeline.run(gem_bs.get_signal_clone())?;
	}
	gem_bs.insert_asset("gembs_reference", &gref, AssetType::Derived);			
	gem_bs.insert_asset("gembs_reference_fai", &gref_fai, AssetType::Derived);			
	gem_bs.insert_asset("gembs_reference_gzi", &gref_gzi, AssetType::Derived);			
	gem_bs.insert_asset("contig_md5", &ctg_md5, AssetType::Derived);	
	gem_bs.check_signal()
}

fn add_make_index_task(gem_bs: &mut GemBS, idx_name: &str, desc: &str, command: &str) {
	let gref = if let Some(x) = gem_bs.get_asset("gembs_reference") { x.idx() } else { panic!("gembs_reference not found")};
	let index_asset = if let Some(x) = gem_bs.get_asset(idx_name) { x } else { panic!("{} not found", idx_name)};
	let index = index_asset.idx();
	let (id, desc, command, args) = (idx_name.to_string(), desc.to_string(), Command::Index, command.to_string());
	let (log_name, log_path) = assets::derive_log_asset(&id, index_asset.path());
	let log_index = gem_bs.insert_asset(&log_name, &log_path, AssetType::Log);
	let cores = gem_bs.get_config_int(Section::Index, "cores").map(|x| x as usize);
	let memory = gem_bs.get_config_memsize(Section::Index, "memory");
	let time = gem_bs.get_config_joblen(Section::Index, "time").or_else(|| Some(21600.into()));
	let index_task = gem_bs.add_task(&id, &desc, command, &args);
	gem_bs.add_task_inputs(index_task, &[gref]).add_outputs(&[index]).set_log(Some(log_index))
		.add_cores(cores).add_memory(memory).add_time(time);
	gem_bs.get_asset_mut(index).unwrap().set_creator(index_task, &[gref]);
}

fn make_index_tasks(gem_bs: &mut GemBS) -> Result<(), String> {
	match gem_bs.get_config(Section::Index, "need_bs_index") {
		Some(DataValue::Bool(x)) => {
			if *x { add_make_index_task(gem_bs, "index", "Make GEM3 bisulfite index", "--bs-index"); }			
		},
		_ => panic!("No value stored for need_bs_index"),
	}
	match gem_bs.get_config(Section::Index, "need_nonbs_index") {
		Some(DataValue::Bool(x)) => {
			if *x { add_make_index_task(gem_bs, "nonbs_index", "Make GEM3 non-bisulfite bisulfite index", "--nonbs-index"); }	
		},
		_ => panic!("No value stored for need_nonbs_index"),
	}
	gem_bs.check_signal()
}

pub fn check_ref_and_indices(gem_bs: &mut GemBS) -> Result<(), String> {
	check_ref(gem_bs)?;
	check_indices(gem_bs)?;
	make_gem_ref(gem_bs)?;
	check_dbsnp_ref(gem_bs)?;
	make_index_tasks(gem_bs)
}

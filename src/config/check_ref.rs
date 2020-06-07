// Check requirements and presence of reference, index files and index_dir
// Make gemBS reference if required
// Make asset list for refererences, indicies and other associated files

use crate::common::defs::{Section, Metadata, DataValue, Command};
use crate::config::GemBS;
use crate::common::utils::Pipeline;
use crate::common::assets::{AssetType, GetAsset};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::BufRead;

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
	if need_nonbs_index {
		if let Some(DataValue::String(idx)) = gem_bs.get_config(Section::Index, "nonbs_index") { 
			// The file itself does not have to exist, but the parent directory should exist
			let tpath = Path::new(idx);
			let par = if let Some(d) = tpath.parent() {
				if !d.exists() { return Err(format!("Parent directory of non BS index file {} not accessible", idx)); }
				d
			} else { Path::new(".") };
			if gem_bs.get_config(Section::Index, "index_dir").is_none() {
				infer_idx_dir = Some(par.to_str().unwrap().to_string());
			}
		} else { missing_nonbs_index = true; }	 		
	}
	if need_bs_index {
		if let Some(DataValue::String(idx)) = gem_bs.get_config(Section::Index, "index") { 
			// The file itself does not have to exist, but the parent directory should exist
			let tpath = Path::new(idx);
			let par = if let Some(d) = tpath.parent() {
				if !d.exists() { return Err(format!("Parent directory of index file {} not accessible", idx)); }
				d
			} else { Path::new(".") };
			if gem_bs.get_config(Section::Index, "index_dir").is_none() {
				infer_idx_dir = Some(par.to_str().unwrap().to_string());
			}
		} else { missing_index = true; }	 
	}
	if missing_index || missing_nonbs_index {	
		// If we have no index_dir, we put the indices in the current directory
		let idx_dir = if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Index, "index_dir") { x } else { 
			infer_idx_dir = Some(".".to_string());
			"." 
		};
		// Check directory exists
		let tpath = Path::new(idx_dir);
		if !tpath.is_dir() { 
			if let Err(e) = fs::create_dir(tpath) {
				return Err(format!("Could not create index_dir directory {}: {}", idx_dir, e)); 
			}
		} 
		if missing_index {
			let tpath = Path::new(Path::new(reference).file_stem().unwrap()).with_extension("BS.gem");	
			let mut idx = PathBuf::from(idx_dir);
			idx.push(tpath);		
			infer_idx = Some(idx.to_str().unwrap().to_string());
		}
		if missing_nonbs_index {
			let tpath = Path::new(Path::new(reference).file_stem().unwrap()).with_extension("gem");	
			let mut idx = PathBuf::from(idx_dir);
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
		gem_bs.set_config(Section::Index, "index_dir", DataValue::String(x));
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
	Ok(())	
}

fn make_gem_ref(gem_bs: &mut GemBS) -> Result<(), String> {	
	let reference = gem_bs.get_reference()?;
	let index_dir = if let Some(DataValue::String(idx)) = gem_bs.get_config(Section::Index, "index_dir") { idx } else { panic!("Internal error - missing index_dir") }; 
	let tpath = Path::new(Path::new(reference).file_stem().unwrap()).with_extension("gemBS.ref");	
	let mut gref = PathBuf::from(index_dir);
	gref.push(tpath);
	let gref_fai = gref.clone().with_extension("ref.fai");
	let gref_gzi = gref.clone().with_extension("ref.gzi");
	let tpath = Path::new(Path::new(reference).file_stem().unwrap()).with_extension("gemBS.contig_md5");
	let mut ctg_md5 = PathBuf::from(index_dir);
	ctg_md5.push(tpath);
	// Create gemBS reference if it does not already exist		
	if !(gref.exists() && ctg_md5.exists()) {
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
		pipeline.run(gem_bs)?;
	}
	// Create faidx index if required		
	if !(gref_fai.exists() && gref_gzi.exists()) {
		info!("Creating gemBS faidx index");
		let faidx_args = vec!("faidx", gref.to_str().unwrap());
		let samtools_path = gem_bs.get_exec_path("samtools");
		let mut pipeline = Pipeline::new();
		pipeline.add_stage(&samtools_path, Some(faidx_args.iter()))
				.add_output(&gref_fai).add_output(&gref_gzi);
		pipeline.run(gem_bs)?;
	}
	gem_bs.insert_asset("gembs_reference", &gref, AssetType::Derived);			
	gem_bs.insert_asset("gembs_reference_fai", &gref_fai, AssetType::Derived);			
	gem_bs.insert_asset("gembs_reference_gzi", &gref_gzi, AssetType::Derived);			
	gem_bs.insert_asset("contig_md5", &ctg_md5, AssetType::Derived);			
	Ok(())
}

fn add_make_index_task(gem_bs: &mut GemBS, idx_name: &str, desc: &str, command: &str) {
	let gref = if let Some(x) = gem_bs.get_asset("gembs_reference") { x.idx() } else { panic!("gembs_reference not found")};
	let index = if let Some(x) = gem_bs.get_asset(idx_name) { x.idx() } else { panic!("{} not found", idx_name)};
	let (id, desc, command, args) = (idx_name.to_string(), desc.to_string(), Command::Index, command.to_string());
	let index_task = gem_bs.add_task(&id, &desc, command, &args, vec!(gref), vec!(index));
	gem_bs.get_asset_mut(index).unwrap().set_creator(index_task);
}

fn make_index_tasks(gem_bs: &mut GemBS) -> Result<(), String> {
	match gem_bs.get_config(Section::Index, "need_bs_index") {
		Some(DataValue::Bool(x)) => {
			if *x { add_make_index_task(gem_bs, "index", "Make GEM3 bisulfite index", "-b"); }			
		},
		_ => panic!("No value stored for need_bs_index"),
	}
	match gem_bs.get_config(Section::Index, "need_nonbs_index") {
		Some(DataValue::Bool(x)) => {
			if *x { add_make_index_task(gem_bs, "nonbs_index", "Make GEM3 non-bisulfite bisulfite index", "-n"); }	
		},
		_ => panic!("No value stored for need_nonbs_index"),
	}
	Ok(())
}

pub fn check_ref_and_indices(gem_bs: &mut GemBS) -> Result<(), String> {
	check_ref(gem_bs)?;
	check_indices(gem_bs)?;
	make_gem_ref(gem_bs)?;
	make_index_tasks(gem_bs)
}

// Check requirements and presence of reference, index files and index_dir

use crate::common::defs::{Section, Metadata, DataValue};
use crate::config::GemBS;
use std::path::{Path, PathBuf};

fn check_ref(gem_bs: &GemBS) -> Result<(), String> {
	// Check reference file
	let reference = gem_bs.get_reference()?;
	let tpath = Path::new(reference);
	if !tpath.exists() { return Err(format!("Reference file {} does not exist or is not accessible", reference)); }
	trace!("Reference file {} found", reference);
	
	// Check extra references - these are not required, but if specified in the config file, the file should be present
	if let Some(DataValue::String(ref_file)) = gem_bs.get_config(Section::Index, "extra_references") { 
		let tpath = Path::new(ref_file);
		if !tpath.exists() { return Err(format!("Extra references file {} does not exist or is not accessible", ref_file)); }
		trace!("Extra references file {} found", ref_file);
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
		if !tpath.is_dir() { return Err(format!("Index_dir directory {} not accessible", idx_dir)); } 
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
		trace!("Setting index to {}", x);
		gem_bs.set_config(Section::Index, "index", DataValue::String(x));
	}
	if let Some(x) = infer_nonbs_idx {
		trace!("Setting non BS index to {}", x);
		gem_bs.set_config(Section::Index, "nonbs_index", DataValue::String(x));
	}
	if let Some(x) = infer_idx_dir {
		trace!("Setting index_dir to {}", x);
		gem_bs.set_config(Section::Index, "index_dir", DataValue::String(x));
	}
	trace!("Setting need_bs_index to {}", need_bs_index);
	gem_bs.set_config(Section::Index, "need_bs_index", DataValue::Bool(need_bs_index));
	trace!("Setting need_non_bs_index to {}", need_bs_index);
	gem_bs.set_config(Section::Index, "need_nonbs_index", DataValue::Bool(need_nonbs_index));
	Ok(())	
}

pub fn check_ref_and_indices(gem_bs: &mut GemBS) -> Result<(), String> {
	check_ref(gem_bs)?;
	check_indices(gem_bs)
}

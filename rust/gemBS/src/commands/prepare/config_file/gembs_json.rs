use std::path::{Path, PathBuf};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use lazy_static::lazy_static;

use utils::compress;
use crate::common::defs::{Section, DataValue, VarType};
use crate::commands::prepare::config_file::KnownVarList;
use crate::config::GemBS;


#[derive(Deserialize, Debug)]
struct GemBSJson(HashMap<Section, HashMap<String, String>>);

fn check_and_assign_var(gem_bs: &mut GemBS, k: &str, vt: VarType, section: Section, dat: &str, p: &Path) -> Result<(), String> {
	lazy_static! {
	    static ref INDEX_PATH_VARS: HashSet<&'static str> = {
	        let mut m = HashSet::new();
			m.insert("index");
			m.insert("nonbs_index");
			m.insert("reference");
			m.insert("extra_references");
			m.insert("contig_sizes");
			m.insert("dbsnp_files");
			m.insert("dbsnp_index");
	        m
	    };
	}
	// Check if already set.  If so then ignore new setting
	if gem_bs.get_config_strict(section, k).is_none() {
		let mut val = dat.to_owned();
		if INDEX_PATH_VARS.contains(k.to_lowercase().as_str()) {
			if let Some(idir) = gem_bs.get_config_str(Section::Mapping, "index_dir") {
				let tp: PathBuf = [Path::new(idir), Path::new(&val)].iter().collect();
				val = format!("{}", tp.display());
			}
		}
		let rv = match vt {
			VarType::String => Some(DataValue::String(val)),
			VarType::FloatVec => { 
				let mut v = Vec::new();					
				if let DataValue::Float(val) = DataValue::from_str(&val, VarType::Float)? { v.push(val); }
				Some(DataValue::FloatVec(v))					
			},
			VarType::IntVec => { 
				let mut v = Vec::new();					
				if let DataValue::Int(val) = DataValue::from_str(&val, VarType::Int)? { v.push(val); }
				Some(DataValue::IntVec(v))					
			},
			VarType::StringVec => Some(DataValue::StringVec(vec!(val))),					
			_ => Some(DataValue::from_str(&val, vt)?),
		};
		if let Some(var) = rv { gem_bs.set_config(section, k, var); }

	} else { warn!("Ignored setting [{:?}]{} from gemBS JSON file {}", section, k, p.display()) }
	Ok(())
}

fn process_gembs_json_file(gem_bs: &mut GemBS, kv_list: &KnownVarList, path: &Path) -> Result<(), String> {
	
	let rdr = compress::open_bufreader(path).map_err(|e| format!("Could not open gemBS JSON file {}: {}", path.display(), e))?;
	let json_data: GemBSJson = serde_json::from_reader(rdr).map_err(|e| format!("Could not parse JSON metadata file {}: {}", path.display(), e))?;
	for (section, data) in json_data.0.iter() {
		for(k, dat) in data.iter() {
			if let Some(vt) = kv_list.check_vtype(k, *section) {
				check_and_assign_var(gem_bs, k, vt, *section, dat, path)?;
			} else { warn!("Unknown key {} for section {:?} in gemBS JSON file {}", k, section, path.display()); }	
		}
	}
	Ok(())
}

// Check for existence of gemBS_index.json file in index_dir 
// and if so, process it. 
pub fn check_gembs_json(gem_bs: &mut GemBS, kv_list: &KnownVarList) -> Result<(), String> {
	if let Some(DataValue::String(idx_dir)) = gem_bs.get_config(Section::Mapping, "index_dir") {
		let p:PathBuf = [Path::new(idx_dir), Path::new("gemBS_index.json")].iter().collect();
		if p.exists() { process_gembs_json_file(gem_bs, kv_list, &p)?; }
	}
	Ok(())
}
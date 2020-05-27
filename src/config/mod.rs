// Main configuration structure for gemBS
//
// Holds all of the information from the config files, JSON files, sqlite db etc.
//

use std::collections::HashMap;
use std::path::PathBuf;
use std::env;
use serde::{Serialize, Deserialize};

use crate::common::defs::{Section, Metadata, DataValue};

#[derive(Serialize, Deserialize, Debug)]
enum GemBSHash {
	Config(HashMap<Section, HashMap<String, DataValue>>),
	SampleData(HashMap<String, HashMap<Metadata, DataValue>>),
}

enum DBType {
	DBFile(&'static str),
	DBMem(&'static str),	
}

struct GemBSFiles {
	config_dir: PathBuf,
	json_file: PathBuf,
	gem_bs_root: PathBuf,		
	db: DBType,
}

pub struct GemBS {
	var: Vec<GemBSHash>,
	fs: Option<GemBSFiles>,
}

impl GemBS {
	pub fn new() -> Self {
		let mut gem_bs = GemBS{var: Vec::new(), fs: None};
		gem_bs.var.push(GemBSHash::Config(HashMap::new()));
		gem_bs.var.push(GemBSHash::SampleData(HashMap::new()));	
		gem_bs
	}
	pub fn set_config(&mut self, section: Section, name: &str, val: DataValue) {
		if let GemBSHash::Config(href) = &mut self.var[0] {
			href.entry(section).or_insert_with(HashMap::new);	
			href.get_mut(&section).unwrap().insert(name.to_string(), val);
		} else { panic!("Internal error!"); }
	}
	pub fn set_sample_data(&mut self, dataset: &str, mt: Metadata, val: DataValue) {
		if let GemBSHash::SampleData(href) = &mut self.var[1] {
			href.entry(dataset.to_string()).or_insert_with(HashMap::new);	
			href.get_mut(dataset).unwrap().insert(mt, val);
		} else { panic!("Internal error!"); }
	}
	pub fn get_config(&self, section: Section, name: &str) -> Option<&DataValue> {
		if let GemBSHash::Config(href) = &self.var[0] {
			if let Some(h) = href.get(&section) { return h.get(name); }		
			if let Some(h) = href.get(&Section::Default) { return h.get(name); }
			None
		} else { None }
	}
	pub fn get_config_ref(&self) ->  &HashMap<Section, HashMap<String, DataValue>> {
		if let GemBSHash::Config(href) = &self.var[0] { &href }
		else { panic!("Internal error!"); }
	}	
	pub fn get_sample_data_ref(&self) ->  &HashMap<String, HashMap<Metadata, DataValue>> {
		if let GemBSHash::SampleData(href) = &self.var[1] { &href }
		else { panic!("Internal error!"); }
	}
	// This will panic if called before fs is set, which is fine
	pub fn write_json_config(&self) -> Result<(), String> {
		let json_file = &self.fs.as_ref().unwrap().json_file;
		if let Ok(wr) = compress::get_writer(json_file.to_str(), None) {
			if serde_json::to_writer_pretty(wr, &self.var).is_ok() { trace!("JSON config file written out to {:?}", json_file); }
			else { return Err(format!("Error: Failed to write JSON config file {:?}", json_file)); } 
		} else { return Err(format!("Error: Failed to create JSON config file {:?}", json_file)); } 
		Ok(())
	}
	pub fn setup_fs(&mut self, initial: bool) -> Result<(), String> {
		let config_dir = PathBuf::from(".gemBS");
		let json_file = if let Some(DataValue::String(x)) = self.get_config(Section::Default, "json_file") { PathBuf::from(x) } else { 
			let mut tpath = config_dir.clone();
			tpath.push("gemBS.json");
			tpath
		};
		let gem_bs_root = if let Some(DataValue::String(x)) = self.get_config(Section::Default, "gembs_root") { 
			PathBuf::from(x) 
		} else if let Ok(x) = env::var("GEMBS_ROOT") { 
			PathBuf::from(x) 
		} else { 
			PathBuf::from("/usr/local/lib/gemBS")	
		};
		if !check_root(&gem_bs_root) {
			return Err(format!("Could not find (installation) root directory for gemBS at {:?}.  Use root-dir option or set GEMBS_ROOT environment variable", gem_bs_root));
		}

		let no_db = if let Some(DataValue::Bool(x)) = self.get_config(Section::Default, "no_db") { *x } else { false }; 
		let db = if no_db { DBType::DBMem("gemBS") } else { DBType::DBFile("gemBS.db") };
		if initial {
			if config_dir.exists() {
				if !config_dir.is_dir() {
					return Err(format!("Could not create config directory {:?} as file exists with same name", config_dir));
				}
			} else if std::fs::create_dir(&config_dir).is_err() { return Err(format!("Could not create config directory {:?}", config_dir)); }
		} else {
			if !config_dir.is_dir() { return Err(format!("Config directory {:?} does not exist (or is not accessible)", config_dir)); }
			if !json_file.exists() { return Err(format!("Config JSON file {:?} does not exist (or is not accessible)", json_file)); }
			if let DBType::DBFile(s) = db {
				let mut tpath = config_dir.clone();
				tpath.push(s);
				if !tpath.exists() { return Err(format!("Database file {:?} does not exist (or is not accessible)", tpath)); }
			}
		}
		self.fs = Some(GemBSFiles{config_dir, json_file, gem_bs_root, db});	
		Ok(())
	}
}


fn check_root(path: &PathBuf) -> bool {
	let apps = ["gemBS_cat", "md5_fasta", "mextr", "readNameClean", "snpxtr", "bs_call", "dbSNP_idx",
		"gem-indexer", "gem-mapper", "samtools", "bcftools", "bgzip"];
	
	trace!("Checking for gemBS root in {:?}", path);
	if path.is_dir() {
		let mut tpath = path.clone();
		// Check existence of binaries
		tpath.push("bin");
		if tpath.is_dir() {
			for app in apps.iter() {
				tpath.push(app);				
				if tpath.is_file() { trace!("Checking for {} in {:?}: OK", app, tpath); }
				else { 
					trace!("Checking for {} in {:?}: Not found", app, tpath); 
					return false;
				}
				tpath.pop();
			}
		} else {
			trace!("Bin directory {:?} not found", tpath);
			return false;
		}
		tpath.pop();
		tpath.push("etc/config_scripts");
		if tpath.is_dir() { 
			trace!("Checking for config script directory in {:?}: OK", tpath); 
			true
		} else {
			trace!("Checking for config script directory in {:?}: Not found", tpath); 
			false
		}
	} else { false }
}

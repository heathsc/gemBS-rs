// Main configuration structure for gemBS
//
// Holds all of the information from the config files, JSON files, sqlite db etc.
//

use std::collections::HashMap;
use std::path::PathBuf;
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use serde::{Serialize, Deserialize};
use rusqlite::Connection;
use std::path::Path;
use std::rc::Rc;
use std::cell::RefCell;

use crate::common::defs::{Section, ContigInfo, ContigData, Metadata, DataValue, Command, SIGTERM, SIGINT, SIGQUIT, SIGHUP};
use crate::common::assets::{Asset, AssetList, AssetType, GetAsset};
use crate::common::tasks::{Task, TaskList};

mod database;
pub mod check_ref;
pub mod contig;
pub mod check_map;
pub mod check_call;

#[derive(Serialize, Deserialize, Debug)]
enum GemBSHash {
	Config(HashMap<Section, HashMap<String, DataValue>>),
	SampleData(HashMap<String, HashMap<Metadata, DataValue>>),
	Contig(HashMap<ContigInfo, HashMap<Rc<String>, ContigData>>),
}

enum SQLiteDB {
	File(Connection),
	Mem(Connection),
	None,
}

struct GemBSFiles {
	config_dir: PathBuf,
	json_file: PathBuf,
	gem_bs_root: PathBuf,		
	db_path: PathBuf,
}

pub struct GemBS {
	var: Vec<GemBSHash>,
	fs: Option<GemBSFiles>,
	db: SQLiteDB,
	assets: AssetList,
	tasks: TaskList,
	signal: Arc<AtomicUsize>,
}

impl GemBS {
	pub fn new() -> Self {
		let mut gem_bs = GemBS{var: Vec::new(), fs: None, db: SQLiteDB::None,
		assets: AssetList::new(), tasks: TaskList::new(), signal: Arc::new(AtomicUsize::new(0))};
		let _ = signal_hook::flag::register_usize(signal_hook::SIGTERM, Arc::clone(&gem_bs.signal), SIGTERM);		
		let _ = signal_hook::flag::register_usize(signal_hook::SIGINT, Arc::clone(&gem_bs.signal), SIGINT);		
		let _ = signal_hook::flag::register_usize(signal_hook::SIGQUIT, Arc::clone(&gem_bs.signal), SIGQUIT);		
		let _ = signal_hook::flag::register_usize(signal_hook::SIGHUP, Arc::clone(&gem_bs.signal), SIGHUP);		
		gem_bs.var.push(GemBSHash::Config(HashMap::new()));
		gem_bs.var.push(GemBSHash::SampleData(HashMap::new()));
		gem_bs.var.push(GemBSHash::Contig(HashMap::new()));
		gem_bs
	}
	pub fn get_signal(& self) -> usize {
		self.signal.load(Ordering::Relaxed)
	}
	pub fn set_config(&mut self, section: Section, name: &str, val: DataValue) {
		if let GemBSHash::Config(href) = &mut self.var[0] {
			debug!("Setting {:?}:{} to {:?}", section, name, val);
			href.entry(section).or_insert_with(HashMap::new).insert(name.to_string(), val);
		} else { panic!("Internal error!"); }
	}
	pub fn set_config_path(&mut self, section: Section, name: &str, path: &Path) {
		let tstr = path.to_string_lossy().to_string();
		self.set_config(section, name, DataValue::String(tstr));
	}
	pub fn set_sample_data(&mut self, dataset: &str, mt: Metadata, val: DataValue) {
		if let GemBSHash::SampleData(href) = &mut self.var[1] {
			href.entry(dataset.to_string()).or_insert_with(HashMap::new).insert(mt, val);
		} else { panic!("Internal error!"); }
	}
	fn get_contig_hash_mut(&mut self) -> &mut HashMap<ContigInfo, HashMap<Rc<String>, ContigData>> {
		if let GemBSHash::Contig(href) = &mut self.var[2] {	href } else { panic!("Internal error!"); }
	}
	pub fn get_contig_hash(&self) -> &HashMap<ContigInfo, HashMap<Rc<String>, ContigData>> {
		if let GemBSHash::Contig(href) = &self.var[2] {	href } else { panic!("Internal error!"); }
	}
	pub fn set_contig_def(&mut self, ctg: contig::Contig) {
		let href = self.get_contig_hash_mut();
		let name = Rc::clone(&ctg.name);
		href.entry(ContigInfo::Contigs).or_insert_with(HashMap::new).insert(name, ContigData::Contig(ctg));
	}
	pub fn set_contig_pool_def(&mut self, pool: contig::ContigPool) {
		let href = self.get_contig_hash_mut();
		let name = Rc::clone(&pool.name);
		href.entry(ContigInfo::ContigPools).or_insert_with(HashMap::new).insert(name, ContigData::ContigPool(pool));
	}
	pub fn get_config(&self, section: Section, name: &str) -> Option<&DataValue> {
		if let GemBSHash::Config(href) = &self.var[0] {
			if let Some(h) = href.get(&section) { 
				if let Some(s) = h.get(name) { return Some(s); } 
			}		
			if let Some(h) = href.get(&Section::Default) { return h.get(name); }
		}
		None
	}
	pub fn get_config_bool(&self, section: Section, name: &str) -> bool {
		if let Some(DataValue::Bool(x)) = self.get_config(section, name) { *x } else { false }
	}
	pub fn get_config_ref(&self) ->  &HashMap<Section, HashMap<String, DataValue>> {
		if let GemBSHash::Config(href) = &self.var[0] { &href }
		else { panic!("Internal error!"); }
	}	
	pub fn get_sample_data_ref(&self) ->  &HashMap<String, HashMap<Metadata, DataValue>> {
		if let GemBSHash::SampleData(href) = &self.var[1] { &href }
		else { panic!("Internal error!"); }
	}
	pub fn insert_asset(&mut self, id: &str, path: &Path, asset_type: AssetType) -> usize {
		let ix = self.assets.insert(id, path, asset_type);
		debug!("Inserting Asset({}): {} {} {:?}", ix, id, path.to_string_lossy(), asset_type);
		ix
	}
	pub fn add_task(&mut self, id: &str, desc: &str, command: Command, args: &str, inputs: Vec<usize>, outputs: Vec<usize>) -> usize {
		debug!("Adding task: {} {} {:?} {} in: {:?} out: {:?}", id, desc, command, args, inputs, outputs);
		let v = inputs.clone();
		let task = self.tasks.add_task(id, desc, command, args, inputs, outputs);
		for inp in v.iter() {
			if let Some(x) = self.assets.get_asset(*inp).unwrap().creator() { self.add_parent_child(task, x); }
		}
		task
	}
	pub fn add_parent_child(&mut self, child: usize, parent: usize) {
		self.tasks.get_idx(child).add_parent(parent);
		self.tasks.get_idx(parent).add_child(child);
	}
	pub fn list_tasks(&self) { self.tasks.list_tasks();	}
	// This will panic if called before fs is set, which is fine
	pub fn write_json_config(&self) -> Result<(), String> {
		let json_file = &self.fs.as_ref().unwrap().json_file;
		if let Ok(wr) = compress::get_writer(json_file.to_str(), None) {
			if serde_json::to_writer_pretty(wr, &self.var).is_ok() { trace!("JSON config file written out to {:?}", json_file); }
			else { return Err(format!("Error: Failed to write JSON config file {:?}", json_file)); } 
		} else { return Err(format!("Error: Failed to create JSON config file {:?}", json_file)); } 
		Ok(())
	}
	pub fn create_db_tables(&self) -> Result<(), String> {
		let c = match &self.db {
			SQLiteDB::File(c) => c,
			SQLiteDB::Mem(c) => c,
			SQLiteDB::None => return Err("create_db_tables(): database not yet opened".to_string()),
		};
		database::create_tables(&c)
	}
	pub fn setup_fs(&mut self, initial: bool) -> Result<(), String> {
		let cdir = ".gemBS";
		let json_file = if let Some(DataValue::String(x)) = self.get_config(Section::Default, "json_file") { PathBuf::from(x) } else { 
			[cdir, "gemBS.json"].iter().collect()
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
		let db_path: PathBuf = [cdir, "gemBS.db"].iter().collect();	
		let config_dir = Path::new(cdir);	
		let no_db = if let Some(DataValue::Bool(x)) = self.get_config(Section::Default, "no_db") { *x } else { false }; 
		if initial {
			if config_dir.exists() {
				if !config_dir.is_dir() {
					return Err(format!("Could not create config directory {:?} as file exists with same name", config_dir));
				}
			} else if std::fs::create_dir(&config_dir).is_err() { return Err(format!("Could not create config directory {:?}", config_dir)); }
		} else {
			if !config_dir.is_dir() { return Err(format!("Config directory {:?} does not exist (or is not accessible)", config_dir)); }
			if !json_file.exists() { return Err(format!("Config JSON file {:?} does not exist (or is not accessible)", json_file)); }
			if !no_db && !db_path.exists() { return Err(format!("Database file {:?} does not exist (or is not accessible)", db_path)); }
		}
		let db_conn = database::open_db_connection(&db_path, no_db, initial)?;
		self.db = if no_db { SQLiteDB::Mem(db_conn) } else { SQLiteDB::File(db_conn) };
		self.create_db_tables()?;
		self.fs = Some(GemBSFiles{config_dir: config_dir.to_path_buf(), json_file, gem_bs_root, db_path});	
		Ok(())
	}
	
	pub fn get_reference(&self) -> Result<&str, String> {
		match self.get_config(Section::Index, "reference") {
			Some(DataValue::String(x)) => Ok(x),
			_ => Err("No reference file supplied in config file.   Use key: reference to indicate a valid reference file".to_string()),
		}
	}

	pub fn get_threads(&self, section: Section) -> usize {
		match self.get_config(section, "threads") {
			Some(DataValue::Int(x)) => *x as usize,
			_ => num_cpus::get(),
		}
	}
	
	pub fn get_exec_path(&self, name: &str) -> PathBuf {
		let root = &self.fs.as_ref().unwrap().gem_bs_root;
		let mut tpath = root.clone();
		tpath.push("bin");
		tpath.push(name);
		tpath
	}
}

impl GetAsset<usize> for GemBS {
	fn get_asset(&self, idx: usize) -> Option<&Asset> {
		self.assets.get_asset(idx)
	}
	fn get_asset_mut(&mut self, idx: usize) -> Option<&mut Asset> {
		self.assets.get_asset_mut(idx)
	}
}

impl GetAsset<&str> for GemBS {
	fn get_asset(&self, idx: &str) -> Option<&Asset> {
		self.assets.get_asset(idx)
	}
	fn get_asset_mut(&mut self, idx: &str) -> Option<&mut Asset> {
		self.assets.get_asset_mut(idx)
	}
}



fn check_root(path: &PathBuf) -> bool {
	let apps = ["md5_fasta", "mextr", "readNameClean", "snpxtr", "bs_call", "dbSNP_idx",
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

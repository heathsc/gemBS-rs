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
use std::path::Path;
use std::rc::Rc;

use crate::common::defs::{Section, ContigInfo, ContigData, Metadata, DataValue, Command, SIGTERM, SIGINT, SIGQUIT, SIGHUP, signal_msg};
use crate::common::assets::{Asset, AssetList, AssetType, AssetStatus, GetAsset};
use crate::common::tasks::{Task, TaskList, TaskStatus};
use crate::common::utils::FileLock;
use std::slice;

pub mod check_ref;
pub mod contig;
pub mod check_map;
pub mod check_call;
pub mod check_extract;

#[derive(Serialize, Deserialize, Debug)]
enum GemBSHash {
	Config(HashMap<Section, HashMap<String, DataValue>>),
	SampleData(HashMap<String, HashMap<Metadata, DataValue>>),
	Contig(HashMap<ContigInfo, HashMap<Rc<String>, ContigData>>),
}

struct GemBSFiles {
	config_dir: PathBuf,
	json_file: PathBuf,
	gem_bs_root: PathBuf,		
}

pub struct GemBS {
	var: Vec<GemBSHash>,
	fs: Option<GemBSFiles>,
	assets: AssetList,
	tasks: TaskList,
	contig_pool_digest: Option<String>,
	asset_digest: Option<String>,
	signal: Arc<AtomicUsize>,
}

impl GemBS {
	pub fn new() -> Self {
		let mut gem_bs = GemBS{var: Vec::new(), fs: None, contig_pool_digest: None, asset_digest: None,
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
	pub fn get_signal(&self) -> usize {
		self.signal.load(Ordering::Relaxed)
	}
	pub fn check_signal(&self) -> Result<(), String> {
		match self.get_signal() {
			0 => Ok(()),
			s => Err(format!("Received {} signal.  Closing down", signal_msg(s))),
		}
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
	pub fn add_task(&mut self, id: &str, desc: &str, command: Command, args: &str, inputs: &[usize], outputs: &[usize]) -> usize {
		debug!("Adding task: {} {} {:?} {} in: {:?} out: {:?}", id, desc, command, args, inputs, outputs);
		let task = self.tasks.add_task(id, desc, command, args, inputs, outputs);
		for inp in inputs.iter() {
			if let Some(x) = self.assets.get_asset(*inp).unwrap().creator() { self.add_parent_child(task, x); }
		}
		task
	}
	pub fn get_tasks_iter(&self) -> slice::Iter<'_, Task> { self.tasks.iter() }
	pub fn get_tasks(&self) -> &TaskList { &self.tasks }
	pub fn get_assets(&self) -> &AssetList { &self.assets }
	pub fn add_parent_child(&mut self, child: usize, parent: usize) {
		self.tasks.get_idx(child).add_parent(parent);
		self.tasks.get_idx(parent).add_child(child);
	}	
	// This will panic if called before fs is set, which is fine
	pub fn write_json_config(&self) -> Result<(), String> {
		self.check_signal()?;
		let json_file = &self.fs.as_ref().unwrap().json_file;
		let lock = FileLock::new(json_file).map_err(|e| format!("Error: Could not obtain lock on JSON config file: {}", e))?;
		let writer = lock.writer().map_err(|e| format!("Error: Could not open JSON config file {} for writing: {}", json_file.to_string_lossy(), e))?;
		serde_json::to_writer_pretty(writer, &self.var).map_err(|e| format!("Error: failed to write JSON config file {}: {}", json_file.to_string_lossy(), e))?;
		trace!("JSON config file written out to {:?}", json_file);
		Ok(())
	}
	pub fn read_json_config(&mut self) -> Result<(), String> {
		self.check_signal()?;
		let json_file = &self.fs.as_ref().unwrap().json_file;
		let lock = FileLock::new(json_file).map_err(|e| format!("Error: Could not obtain lock on JSON config file: {}", e))?;
		let reader = lock.reader().map_err(|e| format!("Error: Could not open JSON config file {} for reading: {}", json_file.to_string_lossy(), e))?;
		self.var = serde_json::from_reader(reader).map_err(|e| format!("Error: failed to read JSON config file {}: {}", json_file.to_string_lossy(), e))?;
		trace!("JSON config file read from {:?}", json_file);
		Ok(())
	}

	pub fn setup_fs(&mut self, initial: bool) -> Result<(), String> {
		self.check_signal()?;
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
		let config_dir = Path::new(cdir);	
		if initial {
			if config_dir.exists() {
				if !config_dir.is_dir() {
					return Err(format!("Could not create config directory {:?} as file exists with same name", config_dir));
				}
			} else if std::fs::create_dir(&config_dir).is_err() { return Err(format!("Could not create config directory {:?}", config_dir)); }
		} else {
			if !config_dir.is_dir() { return Err(format!("Config directory {:?} does not exist (or is not accessible)", config_dir)); }
			if !json_file.exists() { return Err(format!("Config JSON file {:?} does not exist (or is not accessible)", json_file)); }
		}
		self.fs = Some(GemBSFiles{config_dir: config_dir.to_path_buf(), json_file, gem_bs_root});	
		Ok(())
	}
	
	pub fn get_reference(&self) -> Result<&str, String> {
		self.check_signal()?;
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
		[root, Path::new("bin"), Path::new(name)].iter().collect()
	}
	
	pub fn get_config_script_path(&self) -> PathBuf {
		let root = &self.fs.as_ref().unwrap().gem_bs_root;
		[root, Path::new("etc"), Path::new("config_scripts")].iter().collect()
	} 

	pub fn get_samples(&self) -> Vec<(String, Option<String>)> {
		let mut bc_set = HashMap::new();
		let href = self.get_sample_data_ref();	
		for (dataset, href1) in href.iter() {
			let name = href1.get(&Metadata::SampleName).and_then(|x| {
				if let DataValue::String(s) = x {Some(s) } else { None }
			});
			if let Some(DataValue::String(bcode)) = href1.get(&Metadata::SampleBarcode) {
				bc_set.insert(bcode, name);
			} else { panic!("No barcode associated with dataset {}", dataset); }
		}	
		let mut sample = Vec::new();
		for (bc, name) in bc_set.iter() {
			let n = if let Some(x) = name {Some((*x).to_owned())} else {None};
			sample.push(((*bc).clone(), n));
		}
		sample
	}
	
	pub fn setup_assets_and_tasks(&mut self) -> Result<(), String> {
		self.check_signal()?;
		check_ref::check_ref_and_indices(self)?;
		self.contig_pool_digest = Some(contig::setup_contigs(self)?);
		check_map::check_map(self)?;
		check_call::check_call(self)?;
		check_extract::check_extract(self)?;
		self.asset_digest = Some(self.assets.get_digest());
		debug!("Asset name digest = {}", self.asset_digest.as_ref().unwrap());
		self.assets.calc_mod_time_ances();
		Ok(())		
	}
	
	pub fn task_status(&self, task: &Task) -> TaskStatus {
		let mut inputs_ready = true;
		let mut latest_input_mod = None;
		for asset in task.inputs().map(|x| self.assets.get_asset(*x).unwrap()) {
			if asset.status() != AssetStatus::Present { inputs_ready = false; }
			latest_input_mod = match (latest_input_mod, asset.mod_time_ances()) {
				(None, None) => None,
				(Some(m), None) => Some(m),
				(None, Some(m)) => Some(m),
				(Some(m), Some(n)) => if n > m { Some(n) } else { Some(m) }
			}
		}
		let mut outputs_ready = true;
		let mut first_output_mod = None;
		for asset in task.outputs().map(|x| self.assets.get_asset(*x).unwrap()) {
			if asset.status() != AssetStatus::Present {
				outputs_ready = false;
				break;
			}
			first_output_mod = match (first_output_mod, asset.mod_time()) {
				(None, None) => None,
				(Some(m), None) => Some(m),
				(None, Some(m)) => Some(m),
				(Some(m), Some(n)) => if n < m { Some(n) } else { Some(m) }
			}
		}
		match (inputs_ready, outputs_ready) {
			(true, true) => {
				match (latest_input_mod, first_output_mod) {
					(Some(m), Some(n)) => if m > n { TaskStatus::Ready } else { TaskStatus::Complete },
					(_, _) => TaskStatus::Complete,
				}
			},
			(false, true) => {
				match (latest_input_mod, first_output_mod) {
					(Some(m), Some(n)) => if m > n { TaskStatus::Waiting } else { TaskStatus::Complete },
					(_, _) => TaskStatus::Complete,
				}				
			},
			(true, false) => TaskStatus::Ready,
			(false, false) => TaskStatus::Waiting,
		}
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



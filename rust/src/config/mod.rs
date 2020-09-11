// Main configuration structure for gemBS
//
// Holds all of the information from the config files, JSON files, sqlite db etc.
//

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::{env,option_env};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use serde::{Serialize, Deserialize};
use std::path::Path;
use std::rc::Rc;
use std::str::FromStr;
use std::time::Instant;

use crate::common::defs::{Section, Metadata, DataValue, JobLen, MemSize, Command, SIGTERM, SIGINT, SIGQUIT, SIGHUP, signal_msg};
use crate::common::assets::{Asset, AssetList, AssetType, AssetStatus, GetAsset};
use crate::common::tasks::{Task, TaskList, TaskStatus, RunningTask};
use crate::common::utils::{FileLock, timed_wait_for_lock, get_phys_memory};
use crate::common::unix_utils::find_exec_path;
use crate::config::contig::{Contig, ContigPool};
use crate::cli::utils::LogLevel;

use std::slice;

pub mod contig;
mod check_ref;
mod check_map;
mod check_report;
mod check_call;
mod check_extract;

#[derive(Serialize, Deserialize, Debug)]
enum GemBSData {
	Config(HashMap<Section, HashMap<String, DataValue>>),
	SampleData(HashMap<String, HashMap<Metadata, DataValue>>),
	Contigs(Vec<Contig>),
	ContigPools(HashMap<Rc<String>, ContigPool>),
}

struct GemBSFiles {
	config_dir: PathBuf,
	config_file: PathBuf,
	gem_bs_root: PathBuf,		
}

pub struct GemBS {
	var: [GemBSData; 4],
	fs: Option<GemBSFiles>,
	assets: AssetList,
	tasks: TaskList,
//	asset_digest: Option<String>,
	total_mem: usize,
	signal: Arc<AtomicUsize>,
	ignore_times: bool,
	ignore_status: bool,
	json_out: Option<String>,
	all: bool,
	slurm: bool,
	slurm_script: Option<String>,
	dry_run: bool,
	verbose: LogLevel,
}

impl GemBS {
	pub fn new() -> Self {
		let total_mem = get_phys_memory().expect("Couldn't get total memory on system");
		let var = [
			GemBSData::Config(HashMap::new()),
			GemBSData::SampleData(HashMap::new()),
			GemBSData::Contigs(Vec::new()),
			GemBSData::ContigPools(HashMap::new()),			
		];
		let gem_bs = GemBS{var, fs: None, 
			ignore_times: false, ignore_status: false, total_mem,
			json_out: None, all: false, slurm: false, slurm_script: None, dry_run: false, verbose: LogLevel::from_str("error").unwrap(),
			assets: AssetList::new(), tasks: TaskList::new(), signal: Arc::new(AtomicUsize::new(0))};
		gem_bs.mask_signals();
		gem_bs
	}
	pub fn mask_signals(&self) {
		let _ = signal_hook::flag::register_usize(signal_hook::SIGTERM, Arc::clone(&self.signal), SIGTERM);		
		let _ = signal_hook::flag::register_usize(signal_hook::SIGINT, Arc::clone(&self.signal), SIGINT);		
		let _ = signal_hook::flag::register_usize(signal_hook::SIGQUIT, Arc::clone(&self.signal), SIGQUIT);		
		let _ = signal_hook::flag::register_usize(signal_hook::SIGHUP, Arc::clone(&self.signal), SIGHUP);				
	}
	pub fn total_mem(&self) -> usize { self.total_mem }
	pub fn get_signal_clone(&self) -> Arc<AtomicUsize> { Arc::clone(&self.signal) }
	pub fn get_signal(&self) -> usize {
		self.signal.load(Ordering::Relaxed)
	}
	pub fn check_signal(&self) -> Result<(), String> {
		match self.get_signal() {
			0 => Ok(()),
			s => Err(format!("Received {} signal.  Closing down", signal_msg(s))),
		}
	}
	pub fn swap_signal(&self, s: usize) -> usize {
		self.signal.swap(s, Ordering::Relaxed)
	}
	pub fn set_config(&mut self, section: Section, name: &str, val: DataValue) {
		if let GemBSData::Config(href) = &mut self.var[0] {
			debug!("Setting {:?}:{} to {:?}", section, name, val);
			href.entry(section).or_insert_with(HashMap::new).insert(name.to_string(), val);
		} else { panic!("Internal error!"); }
	}
	pub fn set_sample_data(&mut self, dataset: &str, mt: Metadata, val: DataValue) {
		if let GemBSData::SampleData(href) = &mut self.var[1] {
			href.entry(dataset.to_string()).or_insert_with(HashMap::new).insert(mt, val);
		} else { panic!("Internal error!"); }
	}
	fn get_contigs_mut(&mut self) -> &mut Vec<Contig> {
		if let GemBSData::Contigs(vref) = &mut self.var[2] { vref } else { panic!("Internal error!"); }
	}
	pub fn get_contigs(&self) -> &Vec<Contig> {
		if let GemBSData::Contigs(vref) = &self.var[2] { vref } else { panic!("Internal error!"); }
	}
	pub fn get_contig_pool_hash(&self) -> &HashMap<Rc<String>, ContigPool> {
		if let GemBSData::ContigPools(href) = &self.var[3] { href } else { panic!("Internal error!"); }
	}
	fn get_contig_pool_hash_mut(&mut self) -> &mut HashMap<Rc<String>, ContigPool> {
		if let GemBSData::ContigPools(href) = &mut self.var[3] { href } else { panic!("Internal error!"); }
	}
	pub fn set_contig_def(&mut self, ctg: contig::Contig) {
		self.get_contigs_mut().push(ctg);
	}
	pub fn set_contig_pool_def(&mut self, pool: contig::ContigPool) {
		let href = self.get_contig_pool_hash_mut();
		let name = Rc::clone(&pool.name);
		href.insert(name, pool);
	}
	pub fn get_config(&self, section: Section, name: &str) -> Option<&DataValue> {
		if let GemBSData::Config(href) = &self.var[0] {
			if let Some(h) = href.get(&section) { 
				if let Some(s) = h.get(name) { return Some(s); } 
			}		
			if let Some(h) = href.get(&Section::Default) { return h.get(name); }
		}
		None
	}
	pub fn get_config_strict(&self, section: Section, name: &str) -> Option<&DataValue> {
		if let GemBSData::Config(href) = &self.var[0] {
			if let Some(h) = href.get(&section) { 
				if let Some(s) = h.get(name) { return Some(s); } 
			}		
		}
		None
	}
	pub fn get_config_bool(&self, section: Section, name: &str) -> bool {
		if let Some(DataValue::Bool(x)) = self.get_config(section, name) { *x } else { false }
	}	
	pub fn get_config_int(&self, section: Section, name: &str) -> Option<isize> {
		if let Some(DataValue::Int(x)) = self.get_config(section, name) { Some(*x) } else { None }
	}	
	pub fn get_config_str(&self, section: Section, name: &str) -> Option<&str> {
		if let Some(DataValue::String(x)) = self.get_config(section, name) { Some(x) } else { None }
	}	
	pub fn get_config_stringvec(&self, section: Section, name: &str) -> Option<&Vec<String>> {
		if let Some(DataValue::StringVec(x)) = self.get_config(section, name) { Some(x) } else { None }
	}	
	pub fn get_config_float(&self, section: Section, name: &str) -> Option<f64> {
		if let Some(DataValue::Float(x)) = self.get_config(section, name) { Some(*x) } else { None }
	}	
	pub fn get_config_joblen(&self, section: Section, name: &str) -> Option<JobLen> {
		if let Some(DataValue::JobLen(x)) = self.get_config(section, name) { Some(*x) } else { None }
	}	
	pub fn get_config_memsize(&self, section: Section, name: &str) -> Option<MemSize> {
		if let Some(DataValue::MemSize(x)) = self.get_config(section, name) { Some(*x) } else { None }
	}	
	pub fn get_sample_data_ref(&self) ->  &HashMap<String, HashMap<Metadata, DataValue>> {
		if let GemBSData::SampleData(href) = &self.var[1] { &href }
		else { panic!("Internal error!"); }
	}
	pub fn insert_asset(&mut self, id: &str, path: &Path, asset_type: AssetType) -> usize {
		let ix = self.assets.insert(id, path, asset_type);
		debug!("Inserting Asset({}): {} {} {:?}", ix, id, path.to_string_lossy(), asset_type);
		ix
	}
	pub fn add_task_inputs(&mut self, task: usize, inputs: &[usize]) -> &mut Task {
		for inp in inputs.iter() {
			if let Some(x) = self.assets.get_asset(*inp).unwrap().creator() { self.add_parent_child(task, x); }
		}
		self.tasks.get_idx(task).add_inputs(inputs)
	}
	pub fn add_task(&mut self, id: &str, desc: &str, command: Command, args: &str) -> usize {
		debug!("Adding task: {} {} {:?} {}", id, desc, command, args);
		self.tasks.add_task(id, desc, command, args)
	}
	pub fn get_tasks_iter(&self) -> slice::Iter<'_, Task> { self.tasks.iter() }
	pub fn get_tasks(&self) -> &TaskList { &self.tasks }
	pub fn get_assets(&self) -> &AssetList { &self.assets }
	pub fn add_parent_child(&mut self, child: usize, parent: usize) {
		self.tasks.get_idx(child).add_parent(parent);
	}	
	// This will panic if called before fs is set, which is fine
	pub fn write_config(&self) -> Result<(), String> {
		self.check_signal()?;
		let config_file = &self.fs.as_ref().unwrap().config_file;		
		let lock = FileLock::new(config_file).map_err(|e| format!("Error: Could not obtain lock on gemBS config file: {}", e))?;
		let mut writer = {
			match find_exec_path("pigz").or_else(|| find_exec_path("gzip")) {
				Some(p) => lock.pipe_writer(&p),
				None => lock.writer(),
			}.map_err(|e| format!("Error: Could not open gemBS config file {} for writing: {}", config_file.to_string_lossy(), e))
		}?;
//		let mut writer = lock.pipe_writer(Path::new("pigz")).map_err(|e| format!("Error: Could not open gemBS config file {} for writing: {}", config_file.to_string_lossy(), e))?;
		debug!("Writing out config file {}", config_file.display());
		let now = Instant::now();
		rmp_serde::encode::write(&mut writer, &self.var).map_err(|e| format!("Error: failed to write RMP config file {}: {}", config_file.to_string_lossy(), e))?;		
		debug!("Config file written out to {} in {}ms", config_file.display(), now.elapsed().as_millis());
		Ok(())
	}
	pub fn read_config(&mut self) -> Result<(), String> {
		self.check_signal()?;
		let config_file = &self.fs.as_ref().unwrap().config_file;
		let lock = timed_wait_for_lock(self.get_signal_clone(), config_file).map_err(|e| format!("Error: Could not obtain lock on JSON config file: {}", e))?;
		let reader = lock.reader().map_err(|e| format!("Error: Could not open JSON config file {} for reading: {}", config_file.to_string_lossy(), e))?;
		debug!("Reading in config file {}", config_file.display());
		let now = Instant::now();
		self.var = rmp_serde::decode::from_read(reader).map_err(|e| format!("Error: failed to read MP config file {}: {}", config_file.to_string_lossy(), e))?;
		debug!("config file read from {} in {}ms", config_file.display(), now.elapsed().as_millis());
		self.check_signal()?;
		Ok(())
	}

	pub fn setup_fs(&mut self, initial: bool) -> Result<(), String> {
		self.check_signal()?;
		let cdir = ".gemBS";
		let config_file = if let Some(DataValue::String(x)) = self.get_config(Section::Default, "config_file") { PathBuf::from(x) } else { 
			[cdir, "gemBS.mp"].iter().collect()
		};
		let compile_root: Option<&'static str> = option_env!("GEMBS_INSTALL_ROOT");
		let gem_bs_root = if let Some(DataValue::String(x)) = self.get_config(Section::Default, "gembs_root") { PathBuf::from(x) } 
		else if let Ok(x) = env::var("GEMBS_ROOT") { PathBuf::from(x) } 
		else if let Some(x) = compile_root { PathBuf::from(x) } 
		else { PathBuf::from("/usr/local/lib/gemBS") };
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
			if !config_dir.is_dir() { return Err(format!("Config directory {} does not exist (or is not accessible)", config_dir.display())); }
			if !config_file.exists() { return Err(format!("Config file {} does not exist (or is not accessible)", config_file.display())); }
		}
		debug!("gem_bs_root set to {}", gem_bs_root.display());
		self.fs = Some(GemBSFiles{config_dir: config_dir.to_path_buf(), config_file, gem_bs_root});	
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
	pub fn get_task_file_path(&self) -> PathBuf {
		[&self.fs.as_ref().unwrap().config_dir, Path::new("gemBS_tasks.json")].iter().collect()
	}
	pub fn get_config_script_path(&self) -> PathBuf {
		let root = &self.fs.as_ref().unwrap().gem_bs_root;
		[root, Path::new("etc"), Path::new("config_scripts")].iter().collect()
	} 
	pub fn get_css_path(&self) -> PathBuf {
		let root = &self.fs.as_ref().unwrap().gem_bs_root;
		[root, Path::new("etc"), Path::new("css")].iter().collect()
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
	
	pub fn setup_assets_and_tasks(&mut self, lock: &FileLock) -> Result<(), String> {
		self.check_signal()?;
		// Assets are inserted in order so we know that a parent asset will always have a lower index than any child
		check_ref::check_ref_and_indices(self)?;
		contig::setup_contigs(self)?;
		check_ref::make_contig_sizes(self)?;		
		check_map::check_map(self)?;
		check_report::check_map_report(self)?;
		check_call::check_call(self)?;
		check_report::check_call_report(self)?;
		check_extract::check_extract(self)?;
		check_report::check_report(self)?;
//		self.asset_digest = Some(self.assets.get_digest());
//		debug!("Asset name digest = {}", self.asset_digest.as_ref().unwrap());
		self.assets.calc_mod_time_ances();
		self.assets.check_delete_status();
		self.rescan_assets_and_tasks(lock)
	}
	pub fn rescan_assets_and_tasks(&mut self, lock: &FileLock) -> Result<(), String> {
		let running = get_running_tasks(lock)?;		
		let running_ids = running.iter().map(|x| self.tasks.find_task(x.id())).fold(HashSet::new(), |mut h, x| {
			if let Some(t) = x { self.tasks[t].outputs().for_each(|y| {h.insert(*y);}) }
			h
		});
		self.assets.recheck_status(&running_ids);
		self.assets.calc_mod_time_ances();
		self.assets.check_delete_status();		
		self.handle_status(&running)?;
		Ok(())				
	}

	fn handle_status(&mut self, running: &[RunningTask]) -> Result<(), String> {
		self.tasks.iter_mut().for_each(|x| x.clear_status());
		self.calc_task_statuses(running);	
		Ok(())	
	}
	fn calc_task_statuses(&mut self, running: &[RunningTask]) {
		let hset: HashSet<&str> = running.iter().fold(HashSet::new(),|mut hr, x| {hr.insert(x.id()); hr});
		let svec: Vec<(usize, TaskStatus)> = self.tasks.iter().map(|x| {
			let mut s = self.task_status(x);
			if hset.contains(x.id()) {
				if s != TaskStatus::Ready && s != TaskStatus::Complete { warn!("Task {} in running queue in {:?} instead of Ready or Complete state", x.id(), s) }
				s = TaskStatus::Running;
			}
			(x.idx(), s)
		}).collect();
		svec.iter().for_each(|(ix, s)| {
			if Some(*s) != self.tasks[*ix].status() {
				let task = &self.tasks[*ix];
				trace!("calc_task_statuses(): switching task {} status from {:?} to {:?}", task.id(), task.status(), *s);
				self.tasks[*ix].set_status(*s);
			}
		});
		
	}
	pub fn task_status(&self, task: &Task) -> TaskStatus {
		if let Some(s) = task.status() { return s; }
		let ignore_times = self.ignore_times();
		let mut inputs_ready = true;
		let mut latest_input_mod = None;
		let mut outputs_ready = true;
		let mut first_output_mod = None;
		if ignore_times {
			let check = |x| match x {
				AssetStatus::Present | AssetStatus::Outdated => 1,
				AssetStatus::Deleted => 2,
				_ => 0,
			};
			for asset in task.inputs().map(|x| self.assets.get_asset(*x).unwrap()) {
				if check(asset.status()) != 1 { inputs_ready = false; break; }
			}
			for asset in task.outputs().map(|x| self.assets.get_asset(*x).unwrap()) {
				if check(asset.status()) == 0 { outputs_ready = false; break; }
			}
		} else {
			for asset in task.inputs().map(|x| self.assets.get_asset(*x).unwrap()) {
				if asset.status() != AssetStatus::Present { inputs_ready = false; }
				latest_input_mod = match (latest_input_mod, asset.mod_time_ances()) {
					(None, None) => None,
					(Some(m), None) => Some(m),
					(None, Some(m)) => Some(m),
					(Some(m), Some(n)) => if n > m { Some(n) } else { Some(m) }
				}
			}
			for asset in task.outputs().map(|x| self.assets.get_asset(*x).unwrap()) {
				if !(asset.status() == AssetStatus::Present || asset.status() == AssetStatus::Incomplete || asset.status() == AssetStatus::Deleted) {
					outputs_ready = false;
					break;
				}
				first_output_mod = match (first_output_mod, asset.mod_time_ances()) {
					(None, None) => None,
					(Some(m), None) => Some(m),
					(None, Some(m)) => Some(m),
					(Some(m), Some(n)) => if n < m { Some(n) } else { Some(m) }
				}
			}
		}
		let tst = |a,b,x| { match (a,b) {
			(Some(m), Some(n)) => if m > n { x } else { TaskStatus::Complete },
			(_, _) => TaskStatus::Complete,			
		}};
		trace!("task_status() {}: {} {} {:?} {:?}", task.id(), inputs_ready, outputs_ready, latest_input_mod, first_output_mod);
		match (inputs_ready, outputs_ready) {
			(true, true) => tst(latest_input_mod, first_output_mod, TaskStatus::Ready),
			(false, true) => tst(latest_input_mod, first_output_mod, TaskStatus::Waiting),
			(true, false) => TaskStatus::Ready,
			(false, false) => TaskStatus::Waiting,
		}
	}
	pub fn set_ignore_times(&mut self, x: bool) { self.ignore_times = x; }
	pub fn ignore_times(&self) -> bool { self.ignore_times }
	pub fn set_ignore_status(&mut self, x: bool) { self.ignore_status = x; }
	pub fn ignore_status(&self) -> bool { self.ignore_status }
	pub fn set_all(&mut self, x: bool) { self.all = x; }
	pub fn all(&self) -> bool { self.all }
	pub fn set_verbose(&mut self, verbose: LogLevel) { self.verbose = verbose; }
	pub fn verbose(&self) -> LogLevel { self.verbose }
	pub fn set_dry_run(&mut self, x: bool) { self.dry_run = x; }
	pub fn dry_run(&self) -> bool { self.dry_run }
	pub fn set_slurm(&mut self, x: bool) { self.slurm = x; }
	pub fn set_slurm_script(&mut self, x: &str) { self.slurm_script = Some(x.to_owned()); }
	pub fn slurm(&self) -> bool { self.slurm }
	pub fn slurm_script(&self) -> &Option<String> { &self.slurm_script }
	pub fn set_json_out(&mut self, s: &str) { self.json_out = Some(s.to_owned()); }
	pub fn json_out(&self) -> Option<&str> { self.json_out.as_deref() }
	pub fn execute_flag(&self) -> bool { !(self.dry_run || self.slurm || self.json_out.is_some() || self.slurm_script.is_some())  }
	pub fn get_required_tasks_from_asset_list(&self, assets: &[usize], com_list: &[Command]) -> Vec<usize> {
		let com_set = com_list.iter().fold(HashSet::new(), |mut hs, x| { hs.insert(*x); hs });
		fn check_reqd(i: usize, reqd: &mut HashSet<usize>, tlist: &mut Vec<usize>, rf: &TaskList, arf: &AssetList, com_set: &HashSet<Command>, ignore: bool) {
			if ! reqd.contains(&i) {
				reqd.insert(i);
				let st = rf[i].status().expect("Status not set for task");
				if ignore || st != TaskStatus::Complete {
					for ix in rf[i].inputs() {
						let asset = arf.get_asset(*ix).unwrap();
						if let Some(j) = asset.creator() {
							if asset.status() != AssetStatus::Present { check_reqd(j, reqd, tlist, rf, arf, com_set, ignore) }
						}
					}
				}
				if (ignore || st != TaskStatus::Complete) && 
					com_set.contains(&rf[i].command()) { tlist.push(i); }
			}
		}
		let mut reqd = HashSet::new();
		let mut tlist = Vec::new();
		let asset_ref = &self.assets;
		let tasks = self.get_tasks();
		let ignore = self.ignore_status();
		for i in assets {
			let asset = asset_ref.get_asset(*i).unwrap(); 
			debug!("Asset check: {:?} {:?} {:?}", asset.id(), asset.path(), asset.status());
			if let Some(j) = asset.creator() {
				if asset.status() != AssetStatus::Present {
					check_reqd(j, &mut reqd, &mut tlist, tasks, asset_ref, &com_set, ignore);
				} 
			}
		}
		tlist.sort();
		if ignore { tlist }
		else {
			let mut tlist1 = Vec::new();
			reqd = HashSet::new();
			for i in tlist.iter() {
				let mut st = tasks[*i].status().unwrap(); // We already checked above that the status for all tasks is present
				if st == TaskStatus::Waiting {
 					st = TaskStatus::Ready;
					for ix in tasks[*i].parents() {
						if !reqd.contains(ix) {
							let st1 = tasks[*ix].status().unwrap();
							if !(st1 == TaskStatus::Complete || st1 == TaskStatus::Running) {
//								println!("Task {}({}) blocked by parent {}({})", tasks[*i].id(), *i, tasks[*ix].id(), *ix);
								st = TaskStatus::Waiting;
								break;
							}
						}
					}
				}
				if st == TaskStatus::Ready || st == TaskStatus::Running {
					reqd.insert(*i);
					tlist1.push(*i);
				}
			} 
			tlist1
		}
	}
	pub fn get_mapping_json_files_for_barcode(&self, barcode: &str) -> Vec<usize> {
		let mut json_files = Vec::new();
		if let Some(t) = self.tasks.find_task(format!("single_map_{}", barcode).as_str()) {
			for i in self.tasks[t].outputs() {
				let asset = self.get_asset(*i).expect("Couldn't get asset");
				if asset.id().ends_with(".json") { json_files.push(asset.idx()) }
			}		
		} else if let Some(t) = self.tasks.find_task(format!("merge-bam_{}", barcode).as_str()) {
			for ix in self.tasks[t].parents() {
				let task = &self.tasks[*ix];
				if task.id().starts_with("map_") {
					for i in task.outputs() {
						let asset = self.get_asset(*i).expect("Couldn't get asset");
						if asset.id().ends_with(".json") { json_files.push(asset.idx()) }
					}		
				}
			}
		} else { panic!("Couldn't find map tasks for barcode"); }
		json_files
	}
}

pub fn get_running_tasks(lock: &FileLock) -> Result<Vec<RunningTask>, String> {
	let running: Vec<RunningTask> = if lock.path().exists() {		
		let reader = lock.reader().map_err(|e| format!("Error: Could not open JSON config file {} for reading: {}", lock.path().to_string_lossy(), e))?;
		serde_json::from_reader(reader).map_err(|e| format!("Error: failed to read JSON config file {}: {}", lock.path().to_string_lossy(), e))?
	} else { Vec::new() };
	Ok(running)
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



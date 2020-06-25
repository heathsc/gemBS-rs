use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, mpsc};
use std::cell::RefCell;
use std::rc::Rc;
use std::{fs, thread, time};
use custom_error::custom_error;

use crate::config::GemBS;
use crate::common::defs::{DataValue, Command, Section, VarType};
use crate::common::tasks::{TaskStatus, RunningTask};
use crate::common::utils::{Pipeline, FileLock};
use crate::common::utils;
use crate::common::assets::{GetAsset};

mod map;
mod index;
mod md5sum;
mod call;

#[derive(Debug)]
struct RunJob {
	id: String,
	task_idx: usize,
	path: PathBuf,	
	runlist: Rc<RefCell<Vec<usize>>>,
	signal: Arc<AtomicUsize>,
}

impl RunJob {
	fn id(&self) -> &str { &self.id }
}

impl Drop for RunJob {
    fn drop(&mut self) {
        trace!("In RunJob Drop for {}", self.path.to_string_lossy());
		match utils::timed_wait_for_lock(Arc::clone(&self.signal), &self.path) {
			Ok(lock) => {
				let running: Option<Vec<RunningTask>> = if let Ok(reader) = lock.reader() {
					if let Ok(x) = serde_json::from_reader(reader) { Some(x) }
					else { None }
				} else { None };
				if let Some(run) = running {
					let nrun: Vec<&RunningTask> = run.iter().filter(|x| x.id() != self.id()).collect();
					if !nrun.is_empty() {
						if let Ok(writer) = lock.writer() {	let _ = serde_json::to_writer_pretty(writer, &nrun); }
					} else { let _ = fs::remove_file(&self.path); }
				} else { warn!("Could not load run queue"); }
				self.runlist.borrow_mut().retain(|i| *i != self.task_idx);
			},
			Err(e) => { warn!("Could not obtain lock for run queue: {}", e); }
		} 
    }
}

#[derive(Debug)]
pub struct Scheduler<'a> {
	running: Rc<RefCell<Vec<usize>>>, // Tasks running on this machine
	lock: Option<FileLock<'a>>,
	task_list: Vec<usize>,
}

custom_error!{pub SchedulerError
	NoTasks = "No tasks",
	NoTasksReady = "No tasks ready to run", 
	WaitingForTasks = "Waiting for tasks on this machine", 
	NoSlots = "No execution slots available",
	NoLock = "No lock obtained",
	TaskTaken = "Task already taken (internal error)",
	IoErr{desc: String} = "IO error: {desc}",
	Signal = "Caught signal - quitting",
}

impl<'a> Scheduler<'a> {
	fn new(task_list: Vec<usize>) -> Self { 
		Scheduler{running: Rc::new(RefCell::new(Vec::new())), lock: None, task_list }
	}
	fn set_task_list(&mut self, task_list: Vec<usize>) { self.task_list = task_list; }
	fn is_empty(&self) -> bool { self.running.borrow().is_empty() }
	fn set_lock(&mut self, lock: FileLock<'a>) { self.lock = Some(lock); }
	fn drop_lock(&mut self) { self.lock = None; }
	fn check_lock(&self) -> bool {self.lock.is_some() }
	fn get_avail_slots(&mut self, gem_bs: &mut GemBS) -> usize {
		let ncpus = num_cpus::get() as isize;
		let mut avail = ncpus;
		let rf = self.running.borrow();
		for ix in rf.iter() {
			let n = match gem_bs.get_tasks()[*ix].command() {
				Command::Index | Command::Map => ncpus,
				Command::Call => {
					if let Some(DataValue::Int(x)) = gem_bs.get_config(Section::Calling, "jobs") { *x } else { 1 }
				},
				Command::Extract => {
					if let Some(DataValue::Int(x)) = gem_bs.get_config(Section::Extract, "jobs") { *x } else { 1 }
				},
				_ => 1,
			};			
			if n < avail { avail -= n }
			else { avail = 0 };
		}
		avail as usize
	}
	
	fn add_task(&mut self, gem_bs: &mut GemBS, ix: usize) -> Result<(), SchedulerError> {
		let lock = self.lock.as_ref().unwrap();
		let mut running: Vec<RunningTask> = if lock.path().exists() {
			let reader = lock.reader()
				.map_err(|e| SchedulerError::IoErr{desc: format!("Error: Could not open JSON config file {} for reading: {}", lock.path().to_string_lossy(), e)})?;
			 serde_json::from_reader(reader)
				.map_err(|e| SchedulerError::IoErr{desc: format!("Error: failed to read JSON config file {}: {}", lock.path().to_string_lossy(), e)})?
		} else { Vec::new() };	
		let task = &gem_bs.get_tasks()[ix];
		if running.iter().any(|x| x.id() == task.id()) { return Err(SchedulerError::TaskTaken) }
		running.push(RunningTask::from_task(task));
		let writer = lock.writer().map_err(|e| SchedulerError::IoErr{desc: format!("Error: Could not open JSON config file {} for writing: {}", lock.path().to_string_lossy(), e)})?;
		serde_json::to_writer_pretty(writer, &running).map_err(|e| SchedulerError::IoErr{desc: format!("Error: failed to write JSON config file {}: {}", lock.path().to_string_lossy(), e)})?;		
		self.running.borrow_mut().push(ix);
		Ok(())
	}
	
	fn get_task(&mut self, gem_bs: &mut GemBS) -> Result<RunJob, SchedulerError> {
		if self.task_list.is_empty() { 
			self.drop_lock();
			return Err(SchedulerError::NoTasks)
		}
		if self.lock.is_none() {return Err(SchedulerError::NoLock)}
		let avail_slots = self.get_avail_slots(gem_bs);
		debug!("Avail slots: {}", avail_slots);
		let mut task_idx = None;
		let mut avail_tasks = true;
		let ncpus = num_cpus::get();
		let mut max = 0;
		if avail_slots > 0 {
			avail_tasks = false;
			for ix in self.task_list.iter() {
				let task = &gem_bs.get_tasks()[*ix];
				if let Some(TaskStatus::Ready) = task.status() {
					avail_tasks = true;
					let n = match gem_bs.get_tasks()[*ix].command() {
						Command::Index | Command::Map => ncpus,
						Command::Call => {
							if let Some(DataValue::Int(x)) = gem_bs.get_config(Section::Calling, "jobs") { *x as usize} else { 1 }
						},
						Command::Extract => {
							if let Some(DataValue::Int(x)) = gem_bs.get_config(Section::Extract, "jobs") { *x as usize} else { 1 }
						},
						_ => 1,
					};
					if n <= avail_slots && n > max { 
						max = n; 
						task_idx = Some(*ix);
					}
				}
			}
		}
		if let Some(x) = task_idx { 
			self.add_task(gem_bs, x)?;
			let task = &gem_bs.get_tasks()[x];
			let runlist = Rc::clone(&self.running);			
			let rj = RunJob{id: task.id().to_string(), task_idx: x, path: self.lock.as_ref().unwrap().path().to_owned(), runlist, signal: gem_bs.get_signal_clone() };
			self.drop_lock();
			Ok(rj) 
		} else {
			self.drop_lock();	
			if avail_tasks { Err(SchedulerError::NoSlots)}
			else if self.running.borrow().is_empty() { Err(SchedulerError::NoTasksReady) }
			else { Err(SchedulerError::WaitingForTasks)}
		}
	}
}

#[derive(Debug)]
pub struct QPipe {
	stages: Vec<(PathBuf, String)>,
	remove: Vec<PathBuf>,
	outputs: Vec<PathBuf>,
	output: Option<PathBuf>,
	log: Option<PathBuf>,
	remove_log: bool,
	sig: Arc<AtomicUsize>,
} 

impl QPipe {
	pub fn new(sig: Arc<AtomicUsize>) -> Self { QPipe{ stages: Vec::new(), remove: Vec::new(), outputs: Vec::new(), output: None, log: None, remove_log: true, sig} }
	pub fn add_stage(&mut self, path: &Path, args: &str) -> &mut Self {
		self.stages.push((path.to_owned(), args.to_owned()));
		self
	}
	pub fn set_remove_log(&mut self, flag: bool) { self.remove_log = flag; } 
	pub fn get_remove_log(&self) -> bool { self.remove_log } 
	pub fn add_remove_file(&mut self, path: &Path) { self.remove.push(path.to_owned()) }
	pub fn add_outputs(&mut self, path: &Path) { self.outputs.push(path.to_owned()) }
	pub fn get_remove_iter(&self) -> std::slice::Iter<'_, PathBuf> {self.remove.iter() }
	pub fn get_outputs_iter(&self) -> std::slice::Iter<'_, PathBuf> {self.outputs.iter() }
	pub fn set_output(&mut self, out: Option<PathBuf>) { self.output = out; }
}

fn handle_job(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>, job: usize) -> Option<QPipe> {
	let task = &gem_bs.get_tasks()[job];
	for p in task.outputs().map(|x| gem_bs.get_asset(*x).expect("Couldn't get output asset").path()) {
		if let Some(par) = p.parent() {
			fs::create_dir_all(par).expect("Could not create required output directories for map command");
		}
	}
	match task.command() {
		Command::Index => Some(index::make_index_pipeline(gem_bs, options, job)),
		Command::Map => Some(map::make_map_pipeline(gem_bs, options, job)),
		Command::MergeBams => Some(map::make_merge_bams_pipeline(gem_bs, options, job)),
		Command::Call => Some(call::make_call_pipeline(gem_bs, options, job)),
		Command::MergeBcfs => Some(call::make_merge_bcfs_pipeline(gem_bs, options, job)),
		Command::IndexBcf => Some(call::make_index_bcf_pipeline(gem_bs, job)),
		Command::MD5Sum => Some(md5sum::make_md5sum_pipeline(gem_bs, job)),
		_ => None, 
	}
}

fn worker_thread(tx: mpsc::Sender<isize>, rx: mpsc::Receiver<Option<QPipe>>, idx: isize) -> Result<(), String> {
	loop {
		match rx.recv() {
			Ok(Some(qpipe)) => {
				let rm_log = qpipe.get_remove_log();
				let rm_list: Vec<_> = qpipe.get_remove_iter().cloned().collect();
				debug!("Worker thread {} received job: {:?}", idx, qpipe);
				let mut pipeline = Pipeline::new();
				for (path, s) in qpipe.stages.iter() { pipeline.add_stage(path, Some(s.split_ascii_whitespace())); }
				let olist: Vec<_> = qpipe.get_outputs_iter().cloned().collect();
				olist.iter().for_each(|x| { pipeline.add_output(x); });
				let log = if let Some(file) = qpipe.log { pipeline.log_file(file.clone()); Some(file) } else { None };				
				match if let Some(path) = qpipe.output { 
					pipeline.out_file(&path); 
					pipeline.run(qpipe.sig)
				} else {
					pipeline.run(qpipe.sig)
				} {
					Ok(_) => {
						debug!("Worker thread {} finished job", idx);
						if rm_log {
							if let Some(lfile) = log {
								if let Err(e) = std::fs::remove_file(&lfile) {
									error!("Could not remove log file {}: {}", lfile.to_string_lossy(), e);
								}
							}
						}
						for p in rm_list.iter() {
							if let Err(e) = std::fs::remove_file(&p) {
								error!("Could not remove file {}: {}", p.to_string_lossy(), e);
							}
						} 
						tx.send(idx).expect("Error sending message to parent")
					},
					Err(e) => {
						tx.send(-(idx + 1)).expect("Error sending message to parent");
						return Err(e);
					},
				}			
			},
			Ok(None) => {
				debug!("Worker thread {} received signal to shutdown", idx);
				break;
			}
			Err(e) => {
				error!("Worker thread {} received error: {}", idx, e);
				break;
			}
		}
	}
	debug!("Worker thread {} shutting down", idx);
	Ok(())
}

struct Worker {
	handle: thread::JoinHandle<Result<(), String>>,
	tx: mpsc::Sender<Option<QPipe>>,
}

pub fn schedule_jobs(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>, task_list: &[usize], asset_ids: &[usize], com_set: &[Command], flock: FileLock) -> Result<(), String> {
	gem_bs.check_signal()?;
	let tlist: Vec<_> = task_list.iter().copied().collect();
	let mut sched = Scheduler::new(tlist);
	let task_path = flock.path();
	sched.set_lock(flock);
	
	// Set up workers
	let (ctr_tx, ctr_rx) = mpsc::channel();
	let mut avail = Vec::new();
	let mut workers = Vec::new();
	let mut jobs = Vec::new();
	for ix in 0..8 {
		let (tx, rx) = mpsc::channel();
		let ctr = mpsc::Sender::clone(&ctr_tx);
		let handle = thread::spawn(move || { worker_thread(ctr, rx, ix)});
		workers.push(Worker{handle, tx});
		avail.push(ix);
	}
	let mut abort = false;
	let mut no_slots = false;
	loop {
		gem_bs.check_signal()?;
		let worker_ix = if no_slots { None } else { avail.pop() };
		if let Some(idx) =  worker_ix {
			if !sched.check_lock() {
				let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?;
				gem_bs.rescan_assets_and_tasks(&flock)?;
				if !asset_ids.is_empty() {
					let tlist = gem_bs.get_required_tasks_from_asset_list(asset_ids, com_set);
					sched.set_task_list(tlist);
				}
				sched.set_lock(flock);
			}	
			match sched.get_task(gem_bs) {
				Ok(job) => {
					if let Some(qpipe) = handle_job(gem_bs, options, job.task_idx) {
						jobs.push((job, idx));					
						workers[idx as usize].tx.send(Some(qpipe)).expect("Error sending new command to worker thread");
					}
				},
				Err(SchedulerError::NoSlots) => {
					debug!("No Slots");
					thread::sleep(time::Duration::from_millis(1000));
					avail.push(idx);
					no_slots = true;
				},
				Err(SchedulerError::WaitingForTasks) => {
					debug!("Waiting for tasks");
					thread::sleep(time::Duration::from_millis(1000));
					avail.push(idx);
				},
				Err(SchedulerError::NoTasks) | Err(SchedulerError::NoTasksReady) => {
					debug!("No tasks to do");
					break;
				},	
				Err(e) => {
					error!("Scheduler thread received error: {}", e);
					break;					
				},
			}
			match ctr_rx.try_recv() {
				Ok(x) if x >= 0 => {
					debug!("Job completion by worker thread {}", x);
					jobs.retain(|(_, ix)| *ix != x);
					avail.push(x);
					no_slots = false;
				},
				Ok(x) => {
					error!("Error received from worker thread {}", -(x+1));
					abort = true;
					break;
				},
				Err(mpsc::TryRecvError::Empty) => {},
				Err(e) => {
					error!("Scheduler thread received error: {}", e);
					abort = true;
					break;
				}				
			}
							
		} else if !sched.is_empty() {
			match ctr_rx.recv_timeout(time::Duration::from_millis(500)) {
				Ok(x) if x >= 0 => {
					debug!("Job completion by worker thread {}", x);
					jobs.retain(|(_, ix)| *ix != x);
					avail.push(x);
					no_slots = false;
				},
				Ok(x) => {
					error!("Error received from worker thread {}", -(x+1));
					abort = true;
					break;
				},
				Err(mpsc::RecvTimeoutError::Timeout) => {},
				Err(e) => {
					error!("Scheduler thread received error: {}", e);
					abort = true;
					break;
				}				
			}
		}
	}
	while !(abort || sched.is_empty()) {
		gem_bs.check_signal()?;
		match ctr_rx.recv_timeout(time::Duration::from_millis(500)) {
			Ok(x) if x >= 0 => {
				debug!("Job completion by worker thread {}", x);
				jobs.retain(|(_, ix)| *ix != x);
			},
			Ok(x) => {
				error!("Error received from worker thread {}", -(x+1));
				break;
			},
			Err(mpsc::RecvTimeoutError::Timeout) => {},
			Err(e) => {
				error!("Scheduler thread received error: {}", e);
				break;
			}				
		}
	}	
	if !abort {
		for w in workers.drain(..) {
			w.tx.send(None).expect("Error when shutting down thread");
			w.handle.join().expect("Error received from worker at join")?;
		}
	}
	Ok(())
}

pub fn add_command_opts(gem_bs: &GemBS, args: &mut String, sec: Section, opt_list: &[(&'static str, &'static str, VarType)]) {
	for (x, y, t) in opt_list.iter() {
		match t {
			VarType::Bool => if gem_bs.get_config_bool(sec, x) { 
				args.push_str(format!("--{} ", y).as_str()) },
			VarType::Int => if let Some(i) = gem_bs.get_config_int(sec, x) { 
				args.push_str(format!("--{} {} ", y, i).as_str()) },
			VarType::String => if let Some(s) = gem_bs.get_config_str(sec, x) { 
				args.push_str(format!("--{} {} ", y, s).as_str()) },
			VarType::Float => if let Some(z) = gem_bs.get_config_float(sec, x) { 
				args.push_str(format!("--{} {} ", y, z).as_str()) },
			_ => (),
		}
	}
}



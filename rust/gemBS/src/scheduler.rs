
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, mpsc};
use std::cell::RefCell;
use std::rc::Rc;
use std::{fs, thread, time};
use custom_error::custom_error;
use regex::Regex;
use lazy_static::lazy_static;

use crate::config::GemBS;
use crate::common::defs::{DataValue, Command, Section, VarType};
use crate::common::tasks::{TaskStatus, RunningTask};
use crate::common::utils::{Pipeline, FileLock};
use crate::common::utils;
use crate::common::latex_utils::PageSize;
use crate::common::assets::{GetAsset};
use crate::commands::report::{make_map_report, make_call_report, make_report};

use report::{MergeJsonFiles, SampleJsonFiles, CallJsonFiles};

mod map;
mod index;
mod md5sum;
mod extract;
pub mod call;
pub mod report;

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
        trace!("In RunJob Drop for {} {} ({})", self.id, self.task_idx, self.path.display());
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

#[derive(Debug, PartialEq)]
enum SchedState {
	Ready,
	NoSlots,
	Waiting(Option<time::SystemTime>),
	Abort,
}

#[derive(Debug)]
pub struct Scheduler<'a> {
	running: Rc<RefCell<Vec<usize>>>, // Tasks running on this machine
	lock: Option<FileLock<'a>>,
	task_list: Vec<usize>,
	state: SchedState,
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

fn get_requirements(gem_bs: &GemBS, section: Section, default_all: bool) -> (f64, usize) {
	lazy_static! { static ref RE: Regex = Regex::new(r"^(\d+)([kKmMgG]?)$").unwrap(); }

	let ncpus = num_cpus::get() as isize;
	let total_mem = gem_bs.total_mem();
	let n = {
		if let Some(x) = gem_bs.get_config_int(section, "cores") { x.min(ncpus) as f64 }
		else if let Some(x) = gem_bs.get_config_int(section, "jobs") {  ((ncpus as f64) / (x as f64)).max(1.0) }
		else if default_all { ncpus as f64 }
		else { 1.0 }
	};
	let m = { if let Some(x) = gem_bs.get_config_memsize(section, "memory") { x.mem().min(total_mem) } else if default_all { total_mem } else { 0 }};
	(n, m)
}

fn get_merge_bcf_req(gem_bs: &GemBS) -> (f64, usize) {
	let (mut n, _) = get_requirements(gem_bs, Section::Calling, false);
	let threads = gem_bs.get_config_int(Section::Calling, "threads");
	let merge_threads = gem_bs.get_config_int(Section::Calling, "merge_threads").or(threads).map(|x| x as f64);
	if let Some(t) = merge_threads { if t < n { n = t }	}
	(n, 0)
}

fn get_merge_bam_req(gem_bs: &GemBS) -> (f64, usize) {
	let (mut n, _) = get_requirements(gem_bs, Section::Mapping, false);
	let threads = gem_bs.get_config_int(Section::Mapping, "threads");
	let merge_threads = if let Some(t) = gem_bs.get_config_int(Section::Mapping, "merge_threads").or(threads) { t as usize } else { 1 };
	if (merge_threads as f64) < n { n = merge_threads as f64 }
	let total_mem = gem_bs.total_mem();
	let tmem = merge_threads * gem_bs.get_config_memsize(Section::Mapping, "sort_memory").unwrap_or_else(|| 0x30000000.into()).mem();
	let m = if tmem > total_mem { total_mem } else { tmem };
	(n, m)
}

pub fn get_command_req(gem_bs: &GemBS, com: Command) -> (f64, usize) {
	match com {
		Command::Index => get_requirements(gem_bs, Section::Index, true),
		Command::Map => get_requirements(gem_bs, Section::Mapping, true),
		Command::Call => get_requirements(gem_bs, Section::Calling, false),
		Command::IndexBcf => get_requirements(gem_bs, Section::Calling, false),
		Command::Extract => get_requirements(gem_bs, Section::Extract, false),
		Command::MapReport => get_requirements(gem_bs, Section::Report, false),
		Command::CallReport => get_requirements(gem_bs, Section::Report, false),
		Command::Report => get_requirements(gem_bs, Section::Report, false),
		Command::MD5SumMap | Command::MD5SumCall => (1.0, 0), // No special requirement for MD5Sum, and it can not be multithreaded
		Command::MergeBams => get_merge_bam_req(gem_bs),
		Command::MergeBcfs => get_merge_bcf_req(gem_bs),
		Command::MergeCallJsons => (1.0, 0),
	}			
}

impl<'a> Scheduler<'a> {
	fn new(task_list: Vec<usize>) -> Self { 
		Scheduler{running: Rc::new(RefCell::new(Vec::new())), lock: None, task_list, state: SchedState::Ready }
	}
	fn set_task_list(&mut self, task_list: Vec<usize>) { self.task_list = task_list; }
	fn is_empty(&self) -> bool { self.running.borrow().is_empty() }
	fn set_lock(&mut self, lock: FileLock<'a>) { self.lock = Some(lock); }
	fn drop_lock(&mut self) { self.lock = None; }
	fn check_lock(&self) -> bool {self.lock.is_some() }
	fn get_avail_slots_mem(&mut self, gem_bs: &mut GemBS) -> (f64, usize) {
		let ncpus = num_cpus::get() as f64;
		let mut tmem = gem_bs.total_mem();
		let mut avail = ncpus;
		let rf = self.running.borrow();
		for ix in rf.iter() {
			let (n, mem) = get_command_req(gem_bs, gem_bs.get_tasks()[*ix].command());		
			if tmem > mem { tmem -= mem }
			else { tmem = 0 }
			if n < avail { avail -= n }
			else { avail = 0.0 };
		}
		(avail + 0.0001, tmem)
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
		trace!("Scheduling task {} with status {:?}", task.id(), task.status());
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
		let lock = self.lock.as_ref().unwrap();
		let running_jobs = lock.path().exists();
		let lmod_time = match lock.path().metadata() {
			Ok(md) => md.modified().ok(),
			_ => None
		};
		let (avail_slots, avail_mem) = self.get_avail_slots_mem(gem_bs);		
		debug!("Avail slots: {}, avail memory: {:.1} GB", avail_slots, (avail_mem as f64) / 1073741824.0);

		let mut task_idx = None;
		let mut avail_tasks = true;
		let mut max = 0.0;
		if avail_slots > 0.0 {
			avail_tasks = false;
			for ix in self.task_list.iter() {
				let task = &gem_bs.get_tasks()[*ix];
				debug!("Task {}: {:?}", task.id(), task.status());
				if let Some(TaskStatus::Ready) = task.status() {
					avail_tasks = true;
					let (n, mem) = get_command_req(gem_bs, gem_bs.get_tasks()[*ix].command());

					if n <= avail_slots && mem <= avail_mem && n > max { 
						max = n; 
						task_idx = Some(*ix);
					}
				} 
			}
		}
		if let Some(x) = task_idx { 
		  debug!("Tasks available to run");
			self.add_task(gem_bs, x)?;
			let task = &gem_bs.get_tasks()[x];
			let runlist = Rc::clone(&self.running);			
			let rj = RunJob{id: task.id().to_string(), task_idx: x, path: self.lock.as_ref().unwrap().path().to_owned(), runlist, signal: gem_bs.get_signal_clone() };
			self.drop_lock();
			Ok(rj) 
		} else {
		  debug!("No tasks available to run");
			self.drop_lock();	
			if avail_tasks { 
		    debug!("No slots");
				self.state = SchedState::NoSlots;
				Err(SchedulerError::NoSlots)
			} else if !running_jobs { Err(SchedulerError::NoTasksReady) }
			else {
		    debug!("No tasks");
				self.state = SchedState::Waiting(lmod_time);
				Err(SchedulerError::WaitingForTasks)
			}
		}
	}
}

#[derive(Debug)]
pub enum QPipeCom { 
	MapReport((Option<String>, PathBuf, usize, usize, Vec<SampleJsonFiles>)), 
	CallReport((Option<String>, PathBuf, usize, Vec<CallJsonFiles>)),
	Report((Option<String>, PageSize, bool)),
	MergeCallJsons(MergeJsonFiles),
}

#[derive(Debug)]
pub enum QPipeStage {
	External(Vec<(PathBuf, String)>),
	Internal(QPipeCom),
	None,
}

#[derive(Debug)]
pub struct QPipe {
	stages: QPipeStage,
	remove: Vec<PathBuf>,
	outputs: Vec<PathBuf>,
	output: Option<PathBuf>,
	log: Option<PathBuf>,
	remove_log: bool,
	sig: Arc<AtomicUsize>,
} 

impl QPipe {
	pub fn new(sig: Arc<AtomicUsize>) -> Self { QPipe{ stages: QPipeStage::None, remove: Vec::new(), outputs: Vec::new(), output: None, log: None, remove_log: true, sig} }
	pub fn add_stage(&mut self, path: &Path, args: &str) -> &mut Self {
		let stage = (path.to_owned(), args.to_owned());
		match &mut self.stages {
			QPipeStage::None => self.stages = QPipeStage::External(vec!(stage)),
			QPipeStage::External(ref mut s) => s.push(stage),
			_ => panic!("Can't push stages to internal command"),
		}
		self
	}
	pub fn add_com(&mut self, com: QPipeCom) -> &mut Self {
		if let QPipeStage::None = self.stages { self.stages = QPipeStage::Internal(com)	} else { panic!("Can't add internal command to existing pipeline") }
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

fn handle_job(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>, job: usize) -> QPipe {
	let task = &gem_bs.get_tasks()[job];
	for p in task.outputs().map(|x| gem_bs.get_asset(*x).expect("Couldn't get output asset").path()) {
		if let Some(par) = p.parent() {
			fs::create_dir_all(par).expect("Could not create required output directories for command");
		}
	}
	match task.command() {
		Command::Index => index::make_index_pipeline(gem_bs, options, job),
		Command::Map => map::make_map_pipeline(gem_bs, options, job),
		Command::MergeBams => map::make_merge_bams_pipeline(gem_bs, options, job),
		Command::Call => call::make_call_pipeline(gem_bs, job),
		Command::MergeBcfs => call::make_merge_bcfs_pipeline(gem_bs, options, job),
		Command::IndexBcf => call::make_index_bcf_pipeline(gem_bs, job),
		Command::MD5SumMap | Command::MD5SumCall => md5sum::make_md5sum_pipeline(gem_bs, job),
		Command::Extract => extract::make_extract_pipeline(gem_bs, job),
		Command::MapReport => report::make_map_report_pipeline(gem_bs, job),
		Command::CallReport => report::make_call_report_pipeline(gem_bs, job),
		Command::Report => report::make_report_pipeline(gem_bs, job),
		Command::MergeCallJsons => report::make_merge_call_jsons_pipeline(gem_bs, job),
	}
}

fn worker_thread(tx: mpsc::Sender<isize>, rx: mpsc::Receiver<Option<QPipe>>, idx: isize) -> Result<(), String> {
	loop {
		match rx.recv() {
			Ok(Some(qpipe)) => {
				let rm_log = qpipe.get_remove_log();
				let rm_list: Vec<_> = qpipe.get_remove_iter().cloned().collect();
				let out_list: Vec<_> = qpipe.get_outputs_iter().cloned().collect();
				debug!("Worker thread {} received job: {:?}", idx, qpipe);
				let log = &qpipe.log.to_owned();
				let res = match qpipe.stages {
					QPipeStage::External(stages) => {
						let mut pipeline = Pipeline::new();
						for (path, s) in stages.iter() { pipeline.add_stage(path, Some(s.split_terminator('\x1e'))); }
						out_list.iter().for_each(|x| { pipeline.add_output(x); });
						if let Some(file) = log { pipeline.log_file(file.clone()); }
						let opath = &qpipe.output.to_owned();
						if let Some(path) = opath { pipeline.out_filepath(&path); }
						trace!("Launching external pipeline");
						let res = pipeline.run(qpipe.sig);
						trace!("External pipeline ended");
						res
					},
					QPipeStage::Internal(com) => {
						let ret = match com {
							QPipeCom::MergeCallJsons(x) => report::merge_call_jsons(Arc::clone(&qpipe.sig), &qpipe.outputs, &x),
							QPipeCom::MapReport((prj, cdir, thresh, nc, x)) => make_map_report::make_map_report(Arc::clone(&qpipe.sig), &qpipe.outputs, prj, &cdir, thresh, nc, x),
							QPipeCom::CallReport((prj, cdir, nc, x)) => make_call_report::make_call_report(Arc::clone(&qpipe.sig), &qpipe.outputs, prj, &cdir, nc, x),					
							QPipeCom::Report((prj, page_size, pdf)) => make_report::make_report(Arc::clone(&qpipe.sig), &qpipe.outputs, prj, page_size, pdf),
						};
						if ret.is_err() {
							error!("Error returned from internal pipeline command");
							for file in qpipe.outputs.iter() {
								if file.exists() {
									warn!("Removing output file {}", file.to_string_lossy());
									let _ = fs::remove_file(file); 
								}
							}
						} 
						ret
					},
					QPipeStage::None => Err("No pipeline stages".to_string())
				};
				match res {
					Ok(_) => {
						debug!("Worker thread {} finished job", idx);
						if rm_log {
							trace!("Removing log file {:?}", log);
							if let Some(lfile) = log {
								if let Err(e) = std::fs::remove_file(&lfile) {
									error!("Could not remove log file {}: {}", lfile.to_string_lossy(), e);
								}
							}
						}
						for p in rm_list.iter() {
							trace!("Removing file {} after normal task completion", p.display());
							if let Err(e) = std::fs::remove_file(&p) {
								error!("Could not remove file {}: {}", p.to_string_lossy(), e);
							}
						} 
						tx.send(idx).expect("Error sending message to parent")
					},
					Err(e) => {
						tx.send(-(idx + 1)).expect("Error sending message to parent");
						debug!("Worker thread {} shutting down after error {}", idx, e);
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
	ix: usize,
}

pub fn schedule_jobs(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>, task_list: &[usize], asset_ids: &[usize], com_set: &[Command], flock: FileLock) -> Result<(), String> {
	gem_bs.check_signal()?;
	let tlist: Vec<_> = task_list.iter().copied().collect();
	debug!("Schedule_jobs started with {} tasks", tlist.len());
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
		workers.push(Worker{handle, tx, ix: ix as usize});
		avail.push(ix);
	}
	loop {
		if let Err(e) = gem_bs.check_signal() {
			info!("{}", e);
			break;
		}
		let worker_ix = match sched.state {
			SchedState::Ready => avail.pop(),
			SchedState::Waiting(t) => {
				// Check if run file has changed state since we entered SchedState::Waiting
				let mt = match task_path.metadata() {
					Ok(md) => md.modified().ok(),
					_ => None,
				};
				if match (t, mt) {
					(Some(x), Some(y)) => y > x,
					_ => true, 
				} {
					sched.state = SchedState::Ready;
					avail.pop()
				} else { None }
			},
			_ => None,
		};
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
					let qpipe = handle_job(gem_bs, options, job.task_idx);
					jobs.push((job, idx));					
					workers[idx as usize].tx.send(Some(qpipe)).expect("Error sending new command to worker thread");
				},
				Err(SchedulerError::NoSlots) => {
					debug!("No execution slots");
					thread::sleep(time::Duration::from_millis(1000));
					avail.push(idx);
				},
				Err(SchedulerError::WaitingForTasks) | Err(SchedulerError::NoTasksReady) => {
					debug!("Waiting for tasks");
					thread::sleep(time::Duration::from_millis(1000));
					avail.push(idx);
				},
				Err(SchedulerError::NoTasks) => {
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
					sched.state = SchedState::Ready;
				},
				Ok(x) => {
					let x1 = -(x+1);
					error!("Error received from worker thread {}", x1);
					jobs.retain(|(_, ix)| *ix != x1);
					sched.state = SchedState::Abort;
					break;
				},
				Err(mpsc::TryRecvError::Empty) => {},
				Err(e) => {
					error!("Scheduler thread received error: {}", e);
					sched.state = SchedState::Abort;
					break;
				}				
			}
							
		} else if !sched.is_empty() {
			match ctr_rx.recv_timeout(time::Duration::from_millis(1000)) {
				Ok(x) if x >= 0 => {
					debug!("Job completion by worker thread {}", x);
					jobs.retain(|(_, ix)| *ix != x);
					avail.push(x);
					sched.state = SchedState::Ready;
				},
				Ok(x) => {
					let x1 = -(x+1);
					error!("Error received from worker thread {}", x1);
					jobs.retain(|(_, ix)| *ix != x1);
					break;
				},
				Err(mpsc::RecvTimeoutError::Timeout) => {},
				Err(e) => {
					error!("Scheduler thread received error: {}", e);
					sched.state = SchedState::Abort;
					break;
				}				
			}
		} else { thread::sleep(time::Duration::from_secs(5)) }
	}
	// If a signal has been caught, we still want to wait for the jobs to complete if possible
	// but we will abort if a second signal is caught.
	let mut signal = gem_bs.swap_signal(0);
	debug!("Job loop finished - cleaning up");
	if !sched.is_empty() { debug!("Waiting for running jobs to finish") }
	while !sched.is_empty() {
		let s = gem_bs.get_signal();
		if s != 0 {
			if signal == 0 { signal = gem_bs.swap_signal(0) } 
			else { return Err("Received second signal.  Closing down immediately".to_string()) }
		}
		match ctr_rx.recv_timeout(time::Duration::from_millis(1000)) {
			Ok(x) if x >= 0 => {
				debug!("Job completion by worker thread {}", x);
				jobs.retain(|(_, ix)| *ix != x);
			},
			Ok(x) => {
				let x1 = -(x+1);
				error!("Error received from worker thread {}", x1);
				jobs.retain(|(_, ix)| *ix != x1);
			},
			Err(mpsc::RecvTimeoutError::Timeout) => {},
			Err(e) => {
				error!("Scheduler thread received error: {}", e);
				break;
			}				
		}
	}	
	if SchedState::Abort != sched.state && signal == 0 {
		debug!("Waiting for worker threads to terminate");
		for w in workers.drain(..) {
			if w.tx.send(None).is_err() {
				debug!("Error when trying to send shutdown signal to worker thread {}", w.ix);
				sched.state = SchedState::Abort;
				break;
			}
			if w.handle.join().is_err() { 
				debug!("Error received from worker {} at join", w.ix);
				sched.state = SchedState::Abort;
				break;
			}
		}
	}
	if let SchedState::Abort = sched.state { Err("Exiting after error".to_string()) }
	else { Ok(()) }
}

pub fn add_command_opts(gem_bs: &GemBS, args: &mut String, sec: Section, opt_list: &[(&'static str, &'static str, VarType)]) {
	for (x, y, t) in opt_list.iter() {
		match t {
			VarType::Bool => if gem_bs.get_config_bool(sec, x) { 
				args.push_str(format!("--{}\x1e", y).as_str()) },
			VarType::Int => if let Some(i) = gem_bs.get_config_int(sec, x) { 
				args.push_str(format!("--{}\x1e{}\x1e", y, i).as_str()) },
			VarType::IntVec => if let Some(v) = gem_bs.get_config_intvec(sec, x) { 
				if v.len() == 1 { args.push_str(format!("--{}\x1e{}\x1e", y, v[0]).as_str()) }
				else {
					args.push_str(format!("--{}\x1e{}", y, v[0]).as_str());
					for i in v[1..].iter() { args.push_str(format!(",{}", i).as_str()) }
					args.push('\x1e');
				}
			},
			VarType::String => if let Some(s) = gem_bs.get_config_str(sec, x) { 
				args.push_str(format!("--{}\x1e{}\x1e", y, s).as_str()) },
			VarType::Float => if let Some(z) = gem_bs.get_config_float(sec, x) { 
				args.push_str(format!("--{}\x1e{}\x1e", y, z).as_str()) },
			_ => (),
		}
	}
}



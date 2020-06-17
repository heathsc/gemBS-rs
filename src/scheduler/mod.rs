use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::rc::Rc;
use std::sync::Arc;
use std::fs;
use custom_error::custom_error;
use crate::config::GemBS;
use crate::common::defs::{DataValue, Command, Section};
use crate::common::tasks::{TaskStatus, RunningTask};
use crate::common::utils::FileLock;
use crate::common::utils;

#[derive(Debug)]
struct RunJob {
	id: Rc<String>,
	task_idx: usize,
	path: PathBuf,	
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
			},
			Err(e) => { warn!("Could not obtain lock for run queue: {}", e); }
		} 
    }
}


pub struct Scheduler<'a> {
	running: Vec<usize>, // Tasks running on this machine
	lock: Option<FileLock<'a>>,
	task_list: &'a[usize],
}

custom_error!{pub SchedulerError
	NoTasks = "No tasks",
	NoTasksReady = "No tasks ready to run", 
	NoSlots = "No execution slots available",
	NoLock = "No lock obtained",
	TaskTaken = "Task already taken (internal error)",
	IoErr{desc: String} = "IO error: {desc}",
	Signal = "Caught signal - quitting",
}

impl<'a> Scheduler<'a> {
	fn new(task_list: &'a[usize]) -> Self { 
		Scheduler{running: Vec::new(), lock: None, task_list }
	}
	fn set_lock(&mut self, lock: FileLock<'a>) { self.lock = Some(lock); }
	fn drop_lock(&mut self) { self.lock = None; }
	fn get_avail_slots(&mut self, gem_bs: &mut GemBS) -> usize {
		let ncpus = num_cpus::get() as isize;
		let mut avail = ncpus;
		for ix in self.running.iter() {
			let n = match gem_bs.get_tasks()[*ix].command() {
				Command::Index | Command::Map => 0,
				Command::Call => {
					if let Some(DataValue::Int(x)) = gem_bs.get_config(Section::Calling, "jobs") { *x - 1 } else { ncpus - 1 }
				},
				Command::Extract => {
					if let Some(DataValue::Int(x)) = gem_bs.get_config(Section::Extract, "jobs") { *x - 1 } else { ncpus - 1 }
				},
				_ => ncpus - 1,
			};
			if n < avail { avail = n }
		}
		if avail < 0 { 0 } else { avail as usize }
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
		Ok(())
	}
	fn get_task(&mut self, gem_bs: &mut GemBS) -> Result<RunJob, SchedulerError> {
		if self.task_list.is_empty() {return Err(SchedulerError::NoTasks)}
		if self.lock.is_none() {return Err(SchedulerError::NoLock)}
		let avail_slots = self.get_avail_slots(gem_bs);
		trace!("Avail slots: {}", avail_slots);
		let mut task_idx = None;
		let mut avail_tasks = true;
		if avail_slots > 0 {
			avail_tasks = false;
			for ix in self.task_list {
				let task = &gem_bs.get_tasks()[*ix];
				if let Some(TaskStatus::Ready) = task.status() {
					avail_tasks = true;
					match gem_bs.get_tasks()[*ix].command() {
						Command::Index | Command::Map => if self.running.is_empty() { task_idx = Some(*ix); },
						_ => { task_idx = Some(*ix); },
					}
					if task_idx.is_some() { break; }
				}
			}
		}
		if let Some(x) = task_idx { 
			self.add_task(gem_bs, x)?;
			let task = &gem_bs.get_tasks()[x];			
			let rj = RunJob{id: task.id_clone(), task_idx: x, path: self.lock.as_ref().unwrap().path().to_owned(), signal: gem_bs.get_signal_clone() };
			self.drop_lock();
			Ok(rj) 
		} else if avail_tasks { Err(SchedulerError::NoSlots)}
		else { Err(SchedulerError::NoTasksReady)}
	}
}

pub fn schedule_jobs(gem_bs: &mut GemBS, options: &HashMap<&'static str, DataValue>, task_list: &[usize], flock: FileLock) -> Result<(), String> {
	gem_bs.check_signal()?;
	let task_path = gem_bs.get_task_file_path();
	let mut sched = Scheduler::new(task_list);
	sched.set_lock(flock);
	let idx = sched.get_task(gem_bs);
	println!("Aha! {:?}", idx);
	let flock = utils::wait_for_lock(gem_bs, &task_path)?;
	gem_bs.rescan_assets_and_tasks(&flock)?;
	sched.set_lock(flock);
	let idx1 = sched.get_task(gem_bs);
	println!("Aha! {:?}", idx1);
	let flock = utils::wait_for_lock(gem_bs, &task_path)?;
	gem_bs.rescan_assets_and_tasks(&flock)?;
	sched.set_lock(flock);
	let idx2 = sched.get_task(gem_bs);
	println!("Aha! {:?}", idx2);
	println!("Aha! {:?} {:?} {:?}", idx, idx1, idx2);
	Ok(())
}


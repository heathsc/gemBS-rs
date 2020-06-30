use super::defs::{Command, JobLen, MemSize};
use std::ops::{Deref, DerefMut};
use std::slice;
use std::convert::From;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::path::Path;
use std::rc::Rc;
use crate::common::utils;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus { Complete, Ready, Waiting, Running }

#[derive(Debug, Clone)]
pub struct Task {
	id: Rc<String>,
	desc: String,
	barcode: Option<String>, // Barcode of sample that this task applies to (if applicable)
	inputs: Vec<usize>, // Assets required for task
	outputs: Vec<usize>, // Assets generated by task
	parents: Vec<usize>, // Tasks that this one depends on (i.e., that generate the assets in inputs)
	log: Option<usize>,
	cores: Option<usize>,
	memory: Option<MemSize>,
	time: Option<JobLen>,
	idx: usize,
	status: Option<TaskStatus>,
	command: Command,
	args: String,
}

impl Task {
/*	fn new(id: Rc<String>, desc: &str, idx: usize, command: Command, args: &str, inputs: Vec<usize>, outputs: Vec<usize>, log: Option<usize>) -> Self {
		Task{id, desc: desc.to_owned(), barcode: None, idx,
		inputs, outputs, parents: Vec::new(), log,
		status: None, command, args: args.to_owned()}
	} */
	
	fn new(id: Rc<String>, desc: &str, idx: usize, command: Command, args: &str) -> Self {
		Task{id, desc: desc.to_owned(), barcode: None, idx,
		cores: None, memory: None, time: None,
		inputs: Vec::new(), outputs: Vec::new(), parents: Vec::new(), log: None,
		status: None, command, args: args.to_owned()}
	}
	pub fn add_inputs(&mut self, inputs: &[usize]) -> &mut Self { 
		inputs.iter().for_each(|x| self.inputs.push(*x));
		self
	}
	pub fn add_outputs(&mut self, outputs: &[usize]) -> &mut Self { 
		outputs.iter().for_each(|x| self.outputs.push(*x));
		self
	}
	pub fn set_barcode(&mut self, barcode: &str) -> &mut Self {
		self.barcode = Some(barcode.to_owned());
		self 
	}
	pub fn set_log(&mut self, log: Option<usize>) -> &mut Self {
		self.log = log;
		self 
	}
	pub fn set_status(&mut self, status: TaskStatus) -> &mut Self { 
		self.status = Some(status); 
		self
	}
	pub fn add_cores(&mut self, cores: Option<usize>) -> &mut Self { 
		self.cores = cores; 
		self
	}
	pub fn add_memory(&mut self, memory: Option<MemSize>) -> &mut Self { 
		self.memory = memory;
		self
	}
	pub fn add_time(&mut self, time: Option<JobLen>) -> &mut Self { 
		self.time = time; 
		self
	}
	
	pub fn idx(&self) -> usize { self.idx }
	pub fn id(&self) -> &str { &self.id }
	pub fn command(&self) -> Command { self.command }
	pub fn status(&self) -> Option<TaskStatus> { self.status }
	pub fn clear_status(&mut self) { self.status = None; }
	pub fn add_parent(&mut self, ix: usize) { self.parents.push(ix) }
	pub fn parents(&self) -> &[usize] { &self.parents }
	pub fn log(&self) -> Option<usize> { self.log }
	pub fn inputs(&self) -> std::slice::Iter<'_, usize> { self.inputs.iter() }
	pub fn outputs(&self) -> std::slice::Iter<'_, usize> { self.outputs.iter() }
	pub fn args(&self) -> &str { &self.args }
	pub fn barcode(&self) -> Option<&String> { self.barcode.as_ref() }
	pub fn cores(&self) -> Option<usize> { self.cores }
	pub fn memory(&self) -> Option<MemSize> { self.memory }
	pub fn time(&self) -> Option<JobLen> { self.time }
}

#[derive(Debug)]
pub struct TaskList {
	tasks: Vec<Task>,
	thash: HashMap<Rc<String>, usize>,
}

impl TaskList {
	pub fn new() -> Self {
		TaskList{tasks: Vec::new(), thash: HashMap::new() }
	}	
	pub fn add_task(&mut self, id: &str, desc: &str, command: Command, args: &str) -> usize {
		let idx = self.tasks.len();
		let rid = Rc::new(id.to_owned());
		if self.thash.insert(Rc::clone(&rid), idx).is_some() { panic!("Task {} added twice", rid); }
		self.tasks.push(Task::new(rid, desc, idx, command, args));
		idx
	}
	pub fn get_idx(&mut self, idx: usize) -> &mut Task {
		&mut self.tasks[idx]
	}
	pub fn find_task(&self, id: &str) -> Option<usize> {
		self.thash.get(&id.to_string()).copied()
	}
}

impl Deref for TaskList {
	type Target = [Task];
	fn deref(&self) -> &[Task] {
        unsafe {
            slice::from_raw_parts(self.tasks.as_ptr(), self.tasks.len())
        }
	}	
}

impl DerefMut for TaskList {
	fn deref_mut(&mut self) -> &mut [Task] {
        unsafe {
            slice::from_raw_parts_mut(self.tasks.as_mut_ptr(), self.tasks.len())
        }
	}	
}

#[derive(Serialize)]
pub struct JsonTask<'a> {
	id: &'a str,
	command: String,
	args: String,
	inputs: Vec<&'a Path>,
	outputs: Vec<&'a Path>,
	depend: Vec<&'a str>,
	status: TaskStatus,
	cores: usize,
	memory: String,
	time: String,	
}

impl<'a> JsonTask<'a> {
	pub fn new(id: &'a str, command: String, args: String, inputs: Vec<&'a Path>, outputs: Vec<&'a Path>, depend: Vec<&'a str>, status: TaskStatus) -> Self {
		JsonTask{id, command, args, inputs, outputs, depend, status, cores: 1, memory: "1G".to_string(), time: "6:0:0".to_string()}
	}	
	pub fn add_cores(&mut self, cores: usize) -> &mut Self { 
		self.cores = cores;
		self
	}
	pub fn add_memory(&mut self, mem: MemSize) -> &mut Self { 
		self.memory = format!("{}", mem);
		self
	}
	pub fn add_time(&mut self, jlen: JobLen) -> &mut Self { 
		self.time = format!("{}", jlen);
		self
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RunningTask {
	id: Rc<String>,
	command: String,
	args: String,
	caller: String,
}

impl RunningTask {
	pub fn from_task(task: &Task) -> Self {
		RunningTask{id: Rc::clone(&task.id), command: format!("{}",task.command), 
			args: task.args.clone(), caller: utils::get_user_host_string()}
	}
	pub fn id(&self) -> &str { &self.id }

}



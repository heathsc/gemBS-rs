use super::defs::Command;
use std::ops::{Deref, DerefMut};
use std::slice;
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
	inputs: Vec<usize>,
	outputs: Vec<usize>,
	parents: Vec<usize>,
	idx: usize,
	status: Option<TaskStatus>,
	command: Command,
	args: String,
}

impl Task {
	fn new(id: Rc<String>, desc: &str, idx: usize, command: Command, args: &str, inputs: Vec<usize>, outputs: Vec<usize>) -> Self {
		Task{id, desc: desc.to_owned(), idx,
		inputs, outputs, parents: Vec::new(),
		status: None, command, args: args.to_owned()}
	}
	pub fn idx(&self) -> usize { self.idx }
	pub fn id(&self) -> &str { &self.id }
	pub fn id_clone(&self) -> Rc<String> { Rc::clone(&self.id) }
	pub fn command(&self) -> Command { self.command }
	pub fn status(&self) -> Option<TaskStatus> { self.status }
	pub fn set_status(&mut self, status: TaskStatus) { self.status = Some(status); }
	pub fn clear_status(&mut self) { self.status = None; }
	pub fn add_parent(&mut self, ix: usize) { self.parents.push(ix) }
	pub fn parents(&self) -> &[usize] { &self.parents }
	pub fn inputs(&self) -> std::slice::Iter<'_, usize> { self.inputs.iter() }
	pub fn outputs(&self) -> std::slice::Iter<'_, usize> { self.outputs.iter() }
	pub fn args(&self) -> &str { &self.args }
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
	pub fn add_task(&mut self, id: &str, desc: &str, command: Command, args: &str, inputs: &[usize], outputs: &[usize]) -> usize {
		let idx = self.tasks.len();
		let rid = Rc::new(id.to_owned());
		if self.thash.insert(Rc::clone(&rid), idx).is_some() { panic!("Task {} added twice", rid); }
		self.tasks.push(Task::new(rid, desc, idx, command, args, inputs.to_vec(), outputs.to_vec()));
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
}

impl<'a> JsonTask<'a> {
	pub fn new(id: &'a str, command: String, args: String, inputs: Vec<&'a Path>, outputs: Vec<&'a Path>, depend: Vec<&'a str>, status: TaskStatus) -> Self {
		JsonTask{id, command, args, inputs, outputs, depend, status}
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
	pub fn new(id: String, command: String, args: String) -> Self {
		RunningTask{id: Rc::new(id), command, args, caller: utils::get_user_host_string() }
	}
	pub fn from_task(task: &Task) -> Self {
		RunningTask{id: Rc::clone(&task.id), command: format!("{}",task.command), 
			args: task.args.clone(), caller: utils::get_user_host_string()}
	}
	pub fn id(&self) -> &str { &self.id }

}



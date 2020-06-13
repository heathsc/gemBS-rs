use super::defs::Command;
use std::ops::{Deref, DerefMut};
use std::slice;

#[derive(Debug, Clone)]
pub enum TaskStatus { Complete, Ready, Waiting, Running }

#[derive(Debug, Clone)]
pub struct Task {
	id: String,
	desc: String,
	inputs: Vec<usize>,
	outputs: Vec<usize>,
	parents: Vec<usize>,
	children: Vec<usize>,
	idx: usize,
	command: Command,
	args: String,
}

impl Task {
	fn new(id: &str, desc: &str, idx: usize, command: Command, args: &str, inputs: Vec<usize>, outputs: Vec<usize>) -> Self {
		Task{id: id.to_owned(), desc: desc.to_owned(), idx,
		inputs, outputs, parents: Vec::new(), children: Vec::new(),
		command, args: args.to_owned()}
	}
	pub fn idx(&self) -> usize { self.idx }
	pub fn command(&self) -> Command { self.command }
	pub fn add_parent(&mut self, ix: usize) { self.parents.push(ix) }
	pub fn add_child(&mut self, ix: usize) { self.children.push(ix) }
	pub fn parents(&self) -> &[usize] { &self.parents }
	pub fn inputs(&self) -> std::slice::Iter<'_, usize> { self.inputs.iter() }
	pub fn outputs(&self) -> std::slice::Iter<'_, usize> { self.outputs.iter() }
}

#[derive(Debug)]
pub struct TaskList {
	tasks: Vec<Task>,
}

impl TaskList {
	pub fn new() -> Self {
		TaskList{tasks: Vec::new()}
	}
	
	pub fn add_task(&mut self, id: &str, desc: &str, command: Command, args: &str, inputs: &[usize], outputs: &[usize]) -> usize {
		let idx = self.tasks.len();
		self.tasks.push(Task::new(id, desc, idx, command, args, inputs.to_vec(), outputs.to_vec()));
		idx
	}
	pub fn get_idx(&mut self, idx: usize) -> &mut Task {
		&mut self.tasks[idx]
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



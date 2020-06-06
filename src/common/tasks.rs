use super::defs::Command;
use super::assets::Asset;

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
	pub fn add_parent(&mut self, ix: usize) { self.parents.push(ix); }
	pub fn add_child(&mut self, ix: usize) { self.children.push(ix); }
}

#[derive(Debug)]
pub struct TaskList {
	tasks: Vec<Task>,
}

impl TaskList {
	pub fn new() -> Self {
		TaskList{tasks: Vec::new()}
	}
	
	pub fn add_task(&mut self, id: &str, desc: &str, command: Command, args: &str, inputs: Vec<usize>, outputs: Vec<usize>) -> usize {
		let idx = self.tasks.len();
		self.tasks.push(Task::new(id, desc, idx, command, args, inputs, outputs));
		idx
	}
	pub fn get_idx(&mut self, idx: usize) -> &mut Task {
		&mut self.tasks[idx]
	}
	pub fn list_tasks(&self) {
		for task in self.tasks.iter() {
			println!("{:?}", task);
		}
	}
}
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::process::{Command, Stdio, Child};
use std::path::Path;
use std::ffi::OsStr;

pub fn get_inode(name: &str) -> Option<u64> {
   	match fs::metadata(name) {
		Ok(meta) => Some(meta.ino()),
		Err(_) => {
			error!("get_inode() failed for {}", name);
			None
		}	
	}
}

pub struct Pipeline<'a, I, S>
where
	I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
	stage: Vec<(&'a Path, Option<I>)>,
}

impl<'a, I, S> Pipeline<'a, I, S>
where
	I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
	pub fn new() -> Self {
		Pipeline{stage: Vec::new()}
	}
	pub fn add_stage(&mut self, command: &'a Path, args: Option<I>) -> &mut Pipeline<'a, I, S> {
		self.stage.push((command, args));
		self
	}
	pub fn run(&mut self) -> Result<Child, String> {
		let mut len = self.stage.len();
		let mut child: Option<Child> = None;
		for (com, args) in self.stage.drain(..) {
			let mut cc = Command::new(com);
			let mut cc = if let Some(c) = child { cc.stdin(c.stdout.unwrap()) } else {cc.stdin(Stdio::null())};
			len -= 1;
			if len > 0 { cc = cc.stdout(Stdio::piped()); }
			if let Some(a) = args { cc = cc.args(a); }
			child = match cc.spawn() {
				Ok(c) => Some(c),
				Err(_) => return Err(format!("Error - problem launching command {:?}", com)),
			};
		}	
		match child {
			Some(c) => Ok(c),
			None => Err("Error - Empty pipeline".to_string()),
		}
	}
}

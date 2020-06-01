use std::fs;
use std::os::unix::fs::MetadataExt;
use std::process::{Command, Stdio, Child, ChildStdout};
use std::path::Path;
use std::ffi::OsStr;
use std::io::{BufReader, BufRead};

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
	pub fn run(&mut self, capture_output: bool) -> Result<Option<Box<dyn BufRead>>, String> {
		if self.stage.is_empty() { return Err("Error - Empty pipeline".to_string()); }	
		let mut len = self.stage.len();
		let mut cinfo: Vec<(Child, &'a Path)> = Vec::new();
		let mut desc = "Starting pipeline:\n\t".to_string();
		let mut opipe: Option<ChildStdout> = None;
		for (com, args) in self.stage.drain(..) {
			let mut cc = Command::new(com);
			let mut cc = if let Some(c) = cinfo.last_mut() { 
				desc.push_str(format!(" | {}", com.to_string_lossy()).as_str());
				cc.stdin(c.0.stdout.take().unwrap()) 
			} else {
				desc.push_str(format!("{}",com.to_string_lossy()).as_str());
				cc.stdin(Stdio::null())
			};
			let mut arg_vec = Vec::new();
			if let Some(a) = args { 
				for arg in a { 
					desc.push(' ');
					desc.push_str(&(*arg.as_ref().to_str().unwrap()));
					arg_vec.push(arg); 
				}
			}
			len -= 1;
			if len > 0 || capture_output { cc = cc.stdout(Stdio::piped()); }
			if !arg_vec.is_empty() { cc = cc.args(arg_vec.iter())}
			let mut child = match cc.spawn() {
				Ok(c) => { c },
				Err(e) => return Err(format!("Error - problem launching command {}: {}", com.to_string_lossy(), e)),
			};
			trace!("Launched pipeline command {:?}", com);
			if len == 0 && capture_output {
				opipe = Some(child.stdout.take().unwrap());
			}
			cinfo.push((child, com));
		}
		info!("{}", desc);
		let mut err_com = None;
		for (child, com) in cinfo.iter_mut().rev() {
			if err_com.is_some() { 
				trace!("Sending kill signal to {:?} command", com.to_string_lossy()
				);
				let _ = child.kill(); 
			} else {	
				trace!("Waiting for {} to finish", com.to_string_lossy());
				match child.wait() {
					Ok(st) => if !st.success() { err_com = Some(com); },
					Err(_) => err_com = Some(com), 
				}
			}
		}
		match err_com {
			Some(com) => Err(format!("Error running pipeline: {} exited with an error",com.to_string_lossy())),
			None => {
				trace!("Pipeline terminated succesfully");
				if let Some(pipe) = opipe { Ok(Some(Box::new(BufReader::new(pipe)))) } else { Ok(None) }
			},
		}
	}
}

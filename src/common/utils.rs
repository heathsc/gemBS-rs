use std::fs;
use std::os::unix::fs::MetadataExt;
use std::process::{Command, Stdio, Child, ChildStdout};
use std::path::Path;
use std::ffi::OsStr;
use std::io::{BufReader, BufRead};
use std::{thread, time};

use compress::ReadType;
use crate::common::defs::{SIGTERM, SIGINT, SIGQUIT, SIGHUP};
use crate::config::GemBS;

pub fn get_inode(name: &str) -> Option<u64> {
   	match fs::metadata(name) {
		Ok(meta) => Some(meta.ino()),
		Err(_) => {
			error!("get_inode() failed for {}", name);
			None
		}	
	}
}

pub enum PipelineOutput<'a> {
	File(&'a Path),
	Pipe,
	None,
}

pub enum PipelineInput<'a> {
	File(&'a Path),
	Pipe,
	None,
}

pub struct Pipeline<'a, I, S>
where
	I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
	stage: Vec<(&'a Path, Option<I>)>,
	output: PipelineOutput<'a>,
	input: PipelineInput<'a>,
	expected_outputs: Vec<&'a Path>,
}

impl<'a, I, S> Pipeline<'a, I, S>
where
	I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
	pub fn new() -> Self {
		Pipeline{stage: Vec::new(), output: PipelineOutput::None, input: PipelineInput::None, expected_outputs: Vec::new() }
	}
	// Add pipeline stage (command + optional vector of arguments)
	pub fn add_stage(&mut self, command: &'a Path, args: Option<I>) -> &mut Pipeline<'a, I, S> {
		self.stage.push((command, args));
		self
	}
	// Send output of pipeline to file
	pub fn out_file(&mut self, file: &'a Path) -> &mut Pipeline<'a, I, S> {
		self.output = PipelineOutput::File(file);
		self.add_output(file)
	}
	// Get output of pipeline to file
	pub fn in_file(&mut self, file: &'a Path) -> &mut Pipeline<'a, I, S> {
		self.input = PipelineInput::File(file);
		self.add_output(file)
	}
	// Send output of pipeline to BufReader
	pub fn out_pipe(&mut self) -> &mut Pipeline<'a, I, S> {
		self.output = PipelineOutput::Pipe;
		self
	}
	// Add expected output file to pipeline.  If pipeline finished with an error, the expected output files
	// will be deleted
	pub fn add_output(&mut self, file: &'a Path) -> &mut Pipeline<'a, I, S> {
		self.expected_outputs.push(file);
		self
	}
	// Execute the pipeline
	pub fn run(&mut self, gem_bs: &GemBS) -> Result<Option<Box<dyn BufRead>>, String> {
		match self.do_run(gem_bs) {
			Ok(res) => Ok(res),
			Err(e) => {
				for file in self.expected_outputs.iter() { 
					if file.exists() {
						warn!("Removing output file {}", file.to_string_lossy());
						let _ = fs::remove_file(file); 
					}
				}
				Err(e)
			},
		}		
	}
	fn do_run(&mut self, gem_bs: &GemBS) -> Result<Option<Box<dyn BufRead>>, String> {
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
				match self.input {
					PipelineInput::File(file) => {
						desc.push_str(format!("<cat> {} | {}",file.to_string_lossy(), com.to_string_lossy()).as_str());
						let f = match compress::open_reader(file) {
							Ok(x) => x,
							Err(e) => return Err(format!("Couldn't open input file {}: {}", file.to_string_lossy(), e)),
						};
						match f {
							ReadType::File(file) => cc.stdin(file),
							ReadType::Pipe(pipe) => cc.stdin(pipe),
						}
					},
					_ => {
						desc.push_str(format!("{}", com.to_string_lossy()).as_str());
						cc.stdin(Stdio::null())
					}, 
				}
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
			if len > 0 { cc = cc.stdout(Stdio::piped()); } else {
				match self.output {
					PipelineOutput::File(file) => {
						let f = match fs::File::create(file) {
							Ok(x) => x,
							Err(e) => return Err(format!("Couldn't open output file {}: {}", file.to_string_lossy(), e)),
						};
						desc.push_str(format!(" > {}", file.to_string_lossy()).as_str());
						cc = cc.stdout(Stdio::from(f));
					},
					PipelineOutput::Pipe => cc = cc.stdout(Stdio::piped()),
					PipelineOutput::None => (),
				}
			}
			if !arg_vec.is_empty() { cc = cc.args(arg_vec.iter())}
			let mut child = match cc.spawn() {
				Ok(c) => { c },
				Err(e) => return Err(format!("Error - problem launching command {}: {}", com.to_string_lossy(), e)),
			};
			trace!("Launched pipeline command {:?}", com);
			if let PipelineOutput::Pipe = self.output {
				if len == 0 { 
					desc.push_str(" |");
					opipe = Some(child.stdout.take().unwrap()) 
				};
			}
			cinfo.push((child, com));
		}
		info!("{}", desc);
		match wait_sub_proc(gem_bs, &mut cinfo) {
			Some(com) => {
				match gem_bs.get_signal() {
					SIGTERM => Err("Pipeline terminated with a SIGTERM signal".to_string()),
					SIGINT => Err("Pipeline terminated with a SIGINT signal".to_string()),
					SIGHUP => Err("Pipeline terminated with a SIGHUP signal".to_string()),
					SIGQUIT => Err("Pipeline terminated with a SIGQUIT signal".to_string()),
					_ => Err(com),
				}
			},
			None => {
				debug!("Pipeline terminated succesfully");
				if let Some(pipe) = opipe { Ok(Some(Box::new(BufReader::new(pipe)))) } else { Ok(None) }
			},
		}
	}
}

fn wait_sub_proc(gem_bs: &GemBS, cinfo: &mut Vec<(Child, &Path)>) -> Option<String> {
	let mut err_com = None;
	let delay = time::Duration::from_millis(250);
	for (child, com) in cinfo.iter_mut().rev() {
		if err_com.is_some() { 
			trace!("Sending kill signal to {:?} command", com.to_string_lossy());
			let _ = child.kill(); 
		} else {	
			trace!("Waiting for {} to finish", com.to_string_lossy());
			loop {
				if match child.try_wait() {
					Ok(Some(st)) => {
						if !st.success() { err_com = Some(format!("Error from pipeline: {} exited with error", com.to_string_lossy())) }
						true
					},
					Ok(None) => {
						if gem_bs.get_signal() != 0 { let _ = child.kill(); } 
						false
					},
					Err(e) => {
						err_com = Some(format!("Error from pipeline: {} exited with error {}", com.to_string_lossy(), e));
						true
					}, 
				} { break;}
				thread::sleep(delay);
			}
		}
	}
	err_com
}
		


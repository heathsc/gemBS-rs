use std::fs;
use std::os::unix::fs::{symlink, MetadataExt};
use std::process::{Command, Stdio, Child, ChildStdout};
use std::process;
use std::path::{Path, PathBuf};
use std::ffi::{OsString, OsStr};
use std::io::prelude::*;
use std::io::{BufReader, BufRead, BufWriter, ErrorKind};
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{thread, time};
use blake2::{Blake2b, Digest};

use super::compress::ReadType;
use crate::common::defs::{SIGTERM, SIGINT, SIGQUIT, SIGHUP, signal_msg};

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
	log: Option<PathBuf>,
	expected_outputs: Vec<&'a Path>,
}

impl<'a, I, S> Pipeline<'a, I, S>
where
	I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
	pub fn new() -> Self {
		Pipeline{stage: Vec::new(), output: PipelineOutput::None, input: PipelineInput::None, log: None, expected_outputs: Vec::new() }
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
	// Send stderr of pipeline stages to file
	pub fn log_file(&mut self, file: PathBuf) -> &mut Pipeline<'a, I, S> {
		self.log = Some(file);
		self
	}
	pub fn get_log_file(&self) -> &Option<PathBuf> { &self.log }
	
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
	pub fn run(&mut self, sig: Arc<AtomicUsize>) -> Result<Option<Box<dyn BufRead>>, String> {
		let log_file = if let Some(file) = &self.log {
			let f = fs::File::create(file).map_err(|e| format!("Couldn't open output file {}: {}", file.to_string_lossy(), e))?;
			Some(f)
		} else { None };
		self.do_run(sig, log_file).map_err(|e| {
			for file in self.expected_outputs.iter() { 
				if file.exists() {
					warn!("Removing output file {}", file.to_string_lossy());
					let _ = fs::remove_file(file); 
				}
			}
			e
		})
	}
	fn do_run(&mut self, sig: Arc<AtomicUsize>, log: Option<fs::File>) -> Result<Option<Box<dyn BufRead>>, String> {
		if self.stage.is_empty() { return Err("Error - Empty pipeline".to_string()); }	
		let mut len = self.stage.len();
		let mut cinfo: Vec<(Child, &'a Path)> = Vec::new();
		let mut desc = "Starting pipeline:\n\t".to_string();
		let mut opipe: Option<ChildStdout> = None;
		for (com, args) in self.stage.drain(..) {
			let mut cc = Command::new(com);
			let mut cc = if let Some((child, _)) = cinfo.last_mut() { 
				desc.push_str(format!(" | {}", com.to_string_lossy()).as_str());
				cc.stdin(child.stdout.take().unwrap()) 
			} else {
				match self.input {
					PipelineInput::File(file) => {
						let fname = file.to_string_lossy();
						desc.push_str(format!("<cat> {} | {}",fname, com.to_string_lossy()).as_str());
						match super::compress::open_reader(file).map_err(|e| format!("Couldn't open input file {} for pipeline: {}", fname, e))? {
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
			if let Some(lfile) = log.as_ref() { 
				if let Ok(f) = lfile.try_clone() { cc = cc.stderr(f) }
			} 
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
						let fname = file.to_string_lossy();
						let f = fs::File::create(file).map_err(|e| format!("Couldn't open output file {}: {}", fname, e))?;
						desc.push_str(format!(" > {}", fname).as_str());
						cc = cc.stdout(Stdio::from(f));
					},
					PipelineOutput::Pipe => cc = cc.stdout(Stdio::piped()),
					PipelineOutput::None => (),
				}
			}
			if !arg_vec.is_empty() { cc = cc.args(arg_vec.iter())}
			let mut child = cc.spawn().map_err(|e| format!("Error - problem launching command {}: {}", com.to_string_lossy(), e))?;
			trace!("Launched pipeline command {}", com.to_string_lossy());
			if let PipelineOutput::Pipe = self.output {
				if len == 0 { 
					desc.push_str(" |");
					opipe = Some(child.stdout.take().unwrap()) 
				};
			}
			cinfo.push((child, com));
		}
		info!("{}", desc);
		match wait_sub_proc(sig.clone(), &mut cinfo) {
			Some(com) => {
				match get_signal(sig) {
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

fn wait_sub_proc(sig: Arc<AtomicUsize>, cinfo: &mut Vec<(Child, &Path)>) -> Option<String> {
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
						if get_signal(sig.clone()) != 0 { let _ = child.kill(); } 
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
		
pub fn calc_digest<'a>(x: impl Iterator<Item=&'a [u8]>) -> String {
    x.fold(Blake2b::new(), |h, a| h.chain(a))	
		.result().iter().fold(String::new(), |mut s, x| { s.push_str(format!("{:02x}", x).as_str()); s})
}

pub fn get_user_host_string() -> String {
	let pid = process::id();
	let hname = hostname::get().unwrap_or_else(|_| OsString::from("localhost"));
	let user = env::var("USER").unwrap_or_else(|_| {
		let uid = unsafe { libc::getuid() };
		format!("{}", uid)
	});
	format!("{}@{}.{}", user, hname.to_string_lossy(), pid)
}

fn get_lock_path(path: &Path) -> Result<PathBuf, String> {
	let lstring = get_user_host_string();
	let tfile = path.file_name().ok_or(format!("Invalid file {:?} for LockedWriter::new()", path))?.to_string_lossy().to_string();
	let file = if tfile.starts_with('.') {	format!("{}#gemBS_lock", tfile) } else { format!(".{}#gemBS_lock", tfile) };
	let lock_path = match path.parent() {
		Some(parent) => { [parent, Path::new(&file)].iter().collect() },
		None => PathBuf::from(file)
	};
	if let Err(e) = symlink(Path::new(&lstring), &lock_path) {
		return match e.kind() {
			ErrorKind::AlreadyExists => { 
				if let Ok(x) = fs::read_link(&lock_path) { Err(format!("File locked by {}", x.to_string_lossy())) }
				else { Err("File locked".to_string()) }
			},
			_ => Err(format!("{}", e))
		}
	}
	Ok(lock_path)		
}

#[derive(Debug)]
pub struct FileLock<'a> {
	lock_path: PathBuf,
	path: &'a Path,
}

impl<'a> FileLock<'a> {
	pub fn new(path: &'a Path) -> Result<Self, String> {
		let lock_path = get_lock_path(path)?;
		Ok(FileLock{lock_path, path})
	}
	pub fn path(&self) -> &'a Path { self.path }
	pub fn writer(&self) -> Result<Box<dyn Write>, String> {
		let ofile = match fs::File::create(self.path) {
			Err(e) => return Err(format!("Couldn't open {}: {}", self.path.to_string_lossy(), e)),
			Ok(f) => f,
		};
		let writer = Box::new(BufWriter::new(ofile));
		Ok(writer)
	}
	pub fn reader(&self) -> Result<Box<dyn BufRead>, String> {
		let file = match fs::File::open(self.path) {
			Err(e) => return Err(format!("Couldn't open {}: {}", self.path.to_string_lossy(), e)),
			Ok(f) => f,
		};
		let reader = Box::new(BufReader::new(file));
		Ok(reader)
	}
}

pub fn get_signal(sig: Arc<AtomicUsize>) -> usize {
	sig.load(Ordering::Relaxed)
}

pub fn wait_for_lock<'a>(sig: Arc<AtomicUsize>, path: &'a Path) -> Result<FileLock<'a>, String> { timed_wait_for_lock(sig, path) }

pub fn timed_wait_for_lock<'a>(sig: Arc<AtomicUsize>, path: &'a Path) -> Result<FileLock<'a>, String> {
	let delay = time::Duration::from_millis(250);
	let now = time::SystemTime::now();
	let mut signal = sig.swap(0, Ordering::Relaxed);
	let mut message = false;
	loop {
		match FileLock::new(path) {
			Ok(f) => return Ok(f),
			Err(e) => {
				if e.starts_with("File locked") {
					if !message {
						if signal == 0 { warn!("Waiting for lock to allow clean shutdown: {}\nType Ctrl-C twice to quit", e); }
						else { warn!("Waiting for lock to allow clean shutdown: {}\nType Ctrl-C again to quit", e); }
						message = true;
					}
				} else {
					return Err(e);
				}
			},
		}
		let s = get_signal(sig.clone());
		if s != 0 {
			if signal == 0 { 
				signal = sig.swap(0, Ordering::Relaxed);
				message = false;
			} else { return Err(format!("Received {} signal.  Closing down", signal_msg(s))); }
		}
 		if let Ok(t) = now.elapsed() {
			if t.as_secs() >= 300 { return Err("Timed out without obtaining lock".to_string()); }
		}
		thread::sleep(delay);
	}	
}

impl<'a> Drop for FileLock<'a> {
    fn drop(&mut self) {
        trace!("In FileLock Drop for {}", self.path.to_string_lossy());
		let _ = fs::remove_file(&self.lock_path);
    }
}



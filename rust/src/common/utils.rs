use std::fs;
use std::os::unix::fs::{symlink, MetadataExt};
use std::process::{Command, Stdio, Child};
use std::process;
use std::path::{Path, PathBuf};
use std::ffi::{OsString, OsStr};
use std::io::prelude::*;
use std::io::{BufRead, BufWriter, ErrorKind};
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{thread, time};
use std::convert::AsRef;
// use blake2::{Blake2b, Digest};

use super::compress::{open_bufreader, open_pipe_writer};
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

pub fn get_phys_memory() -> Option<usize> {
	let (page_size, num_pages) = unsafe { 
		(libc::sysconf(libc::_SC_PAGE_SIZE), libc::sysconf(libc::_SC_PHYS_PAGES))
	};
	if page_size > 0 && num_pages > 0 {
		Some ((page_size as usize) * (num_pages as usize))
	} else { None }
}

pub enum PipelineOutput<'a> {
	FilePath(&'a Path),
//	File(Option<fs::File>),
	String(Option<String>),
	None,
}

pub enum PipelineInput {
//	File(Option<fs::File>),
	String(String),
	None,
}

pub struct Pipeline<'a, I, S>
where
	I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
	stage: Vec<(&'a Path, Option<I>)>,
	output: PipelineOutput<'a>,
	input: PipelineInput,
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
	// Send output of pipeline to File
//	pub fn out_file(&mut self, file: fs::File) -> &mut Pipeline<'a, I, S> {
//		self.output = PipelineOutput::File(Some(file));
//		self
//	}
//	pub fn in_file(&mut self, file: fs::File) -> &mut Pipeline<'a, I, S> {
//		self.input = PipelineInput::File(Some(file));
//		self
//	}
	pub fn in_string(&mut self, s: String) -> &mut Pipeline<'a, I, S> {
		self.input = PipelineInput::String(s);
		self
	}
	pub fn out_string(&mut self) -> &mut Pipeline<'a, I, S> {
		self.output = PipelineOutput::String(None);
		self
	}
	pub fn out_string_ref(&self) -> Option<&str> { 
		if let PipelineOutput::String(Some(s)) = &self.output { Some(&s) }
		else { None }
	}
	// Send output of pipeline to file at Path
	pub fn out_filepath(&mut self, file: &'a Path) -> &mut Pipeline<'a, I, S> {
		self.output = PipelineOutput::FilePath(file);
		self.add_output(file)
	}
	// Send stderr of pipeline stages to file
	pub fn log_file(&mut self, file: PathBuf) -> &mut Pipeline<'a, I, S> {
		self.log = Some(file);
		self
	}

	// Add expected output file to pipeline.  If pipeline finished with an error, the expected output files
	// will be deleted
	pub fn add_output(&mut self, file: &'a Path) -> &mut Pipeline<'a, I, S> {
		self.expected_outputs.push(file);
		self
	}
	// Execute the pipeline
	pub fn run(&mut self, sig: Arc<AtomicUsize>) -> Result<(), String> {
		let log_file = if let Some(file) = &self.log {
			let f = fs::File::create(file).map_err(|e| format!("Couldn't open output file {}: {}", file.to_string_lossy(), e))?;
			Some(f)
		} else { None };
		self.do_run(sig, log_file).map_err(|e| {
			for file in self.expected_outputs.iter() { 
				debug!("Try to remove output file {}", file.display());
				if file.exists() {
					warn!("Removing output file {}", file.display());
					let _ = fs::remove_file(file); 
				}
			}
			e
		})
	}
	fn do_run(&mut self, sig: Arc<AtomicUsize>, log: Option<fs::File>) -> Result<(), String> {
		if self.stage.is_empty() { return Err("Error - Empty pipeline".to_string()); }	
		let mut len = self.stage.len();
		let mut cinfo: Vec<(Child, &'a Path)> = Vec::new();
		let mut desc = "Launch:\n\t".to_string();
		for (com, args) in self.stage.drain(..) {
			let mut cc = Command::new(com);
			let mut cc = if let Some((child, _)) = cinfo.last_mut() { 
				desc.push_str(format!(" | {}", com.to_string_lossy()).as_str());
				cc.stdin(child.stdout.take().unwrap()) 
			} else {
				desc.push_str(format!("{}", com.to_string_lossy()).as_str());
				if let PipelineInput::String(_) = &self.input {
					trace!("Setting up stdin pipe");
					cc.stdin(Stdio::piped())
//				} else if let PipelineInput::File(optf) = &mut self.input {
//					cc.stdin(Stdio::from(optf.take().expect("No file provided for pipeline input")))
				} else { cc.stdin(Stdio::null()) }
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
				match &mut self.output {
					PipelineOutput::FilePath(file) => {
						let fname = file.to_string_lossy();
						let f = fs::File::create(file).map_err(|e| format!("Couldn't open output file {}: {}", fname, e))?;
						desc.push_str(format!(" > {}", fname).as_str());
						cc = cc.stdout(Stdio::from(f));
					},
//					PipelineOutput::File(optf) => cc = cc.stdout(Stdio::from(optf.take().expect("No file provided for pipeline output"))),
					PipelineOutput::String(_) => cc = cc.stdout(Stdio::piped()),
					PipelineOutput::None => (),
				}
			}
			if !arg_vec.is_empty() { cc = cc.args(arg_vec.iter())}
			let child = cc.spawn().map_err(|e| format!("Error - problem launching command {}: {}", com.to_string_lossy(), e))?;
			trace!("Launched pipeline command {}", com.to_string_lossy());
			cinfo.push((child, com));
		}
		info!("{}", desc);
		if let PipelineInput::String(s) = &self.input {
			// A bit ugly here:
			// We need to take ownership of child not simply have a reference to it
			// otherwise the pipe will not be closed after we've finished.
			// We also take() stdin from child so that we can re-insert child
			// back into cinfo allowing us to wait for the pipeline to terminate
			let (mut child, com) = cinfo.remove(0);
			let mut stdin = child.stdin.take().expect("Failed to open stdin");
			stdin.write_all(s.as_bytes()).expect("Failed to write to child stdin");
			cinfo.insert(0, (child, com));
		}
		
		if let PipelineOutput::String(_) = &self.output {
			let (mut child, com) = cinfo.pop().expect("Empty pipeline");
			let mut stdout = child.stdout.take().expect("Failed to open stdout");
			let mut s = String::new();
			stdout.read_to_string(&mut s).expect("Error reading from pipeline stdout");
			self.output = PipelineOutput::String(Some(s));
			cinfo.push((child, com));
		}
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
				debug!("Process terminated succesfully");
				Ok(())
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
		
//pub fn calc_digest<'a>(x: impl Iterator<Item=&'a [u8]>) -> String {
//   x.fold(Blake2b::new(), |h, a| h.chain(a))	
//		.result().iter().fold(String::new(), |mut s, x| { s.push_str(format!("{:02x}", x).as_str()); s})
//}

pub fn get_user_host_string() -> String {
	let pid = process::id();
	let hname = hostname::get().unwrap_or_else(|_| OsString::from("localhost"));
	let user = env::var("USER").unwrap_or_else(|_| {
		let uid = unsafe { libc::getuid() };
		format!("{}", uid)
	});
	format!("{}@{}.{}", user, hname.to_string_lossy(), pid)
}

fn get_lock_path(path: &Path, force: bool) -> Result<PathBuf, String> {
	let lstring = get_user_host_string();
	let tfile = path.file_name().ok_or(format!("Invalid file {:?} for LockedWriter::new()", path))?.to_string_lossy().to_string();
	let file = if tfile.starts_with('.') {	format!("{}#gemBS_lock", tfile) } else { format!(".{}#gemBS_lock", tfile) };
	let lock_path = match path.parent() {
		Some(parent) => { [parent, Path::new(&file)].iter().collect() },
		None => PathBuf::from(file)
	};
	if force { let _ = fs::remove_file(path); }
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
		let lock_path = get_lock_path(path, false)?;
		Ok(FileLock{lock_path, path})
	}
	pub fn new_force(path: &'a Path) -> Result<Self, String> {
		let lock_path = get_lock_path(path, true)?;
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
	pub fn pipe_writer(&self, prog: &Path) -> Result<Box<dyn Write>, String> {
		open_pipe_writer(self.path, prog).map_err(|e| format!("Couldn't open pipe using {} for writing to {}: {}", prog.display(), self.path.display(), e))
	}
	pub fn reader(&self) -> Result<Box<dyn BufRead>, String> {
		open_bufreader(self.path).map_err(|e| format!("{}", e))
	}
}

pub fn get_signal(sig: Arc<AtomicUsize>) -> usize {
	sig.load(Ordering::Relaxed)
}

pub fn check_signal(sig: Arc<AtomicUsize>) -> Result<(), String> {
	match get_signal(sig) {
		0 => Ok(()),
		s => Err(format!("Received {} signal.  Closing down", signal_msg(s))),
	}
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

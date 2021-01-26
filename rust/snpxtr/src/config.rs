use std::collections::HashSet;
use std::io::{self, Error, ErrorKind};

use r_htslib::BcfSrs;

use crate::dbsnp;

pub fn new_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

#[derive(Default)]
pub struct OutputOpt {
	filename: Option<String>,
	compress: bool,
	compute_md5: bool,
	compute_tbx: bool,
}

impl OutputOpt {
	pub fn new() -> Self { Default::default() }
	pub fn set_filename<S: AsRef<str>>(&mut self, fname: S) -> &mut Self { 
		self.filename = Some(fname.as_ref().to_owned());
		self
	}
	pub fn filename(&self) -> Option<&str> { self.filename.as_ref().map(|s| s.as_str()) }
	pub fn set_compress(&mut self, b: bool) -> &mut Self { self.compress = b; self }
	pub fn set_compute_md5(&mut self, b: bool) -> &mut Self { self.compute_md5 = b; self }
	pub fn set_compute_tbx(&mut self, b: bool) -> &mut Self { self.compute_tbx = b; self }
	pub fn compress(&self) -> bool { self.compress }
	pub fn compute_md5(&self) -> bool { self.compute_md5 }
	pub fn compute_tbx(&self) -> bool { self.compute_tbx }
	// If filename & compress are set, add .gz as suffix unless already present
	// If no filename is set, set compute_md5 and compute_tbx to false, and if compress
	// is set, we check if stdout is a terminal and, if so, the compress option is set to false
	//
	// fix_opts() should be (obviously) run last
	pub fn fix_opts(&mut self) {
		if let Some(fname) = &mut self.filename {
			if self.compress && !fname.ends_with(".gz") { fname.push_str(".gz") }
		} else {
			self.compute_md5 = false;
			self.compute_tbx = false;
			if self.compress && unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 } {
				warn!("Will not send compressed output to terminal");
				self.compress = false;
			}
		}
	}
}

pub struct Config {
	synced_reader: Option<BcfSrs>, 
	threads: usize,
	output: OutputOpt,
	selected_hash: Option<HashSet<String>>,
	dbsnp_file: Option<dbsnp::DBSnpFile>,	
}

impl Config {
	pub fn new(output_opt: OutputOpt, sr: BcfSrs) -> Self { Self {threads: 1, output: output_opt, synced_reader: Some(sr), selected_hash: None, dbsnp_file: None }}
	pub fn set_threads(&mut self, threads: usize) -> &mut Self { self.threads = threads; self }
	pub fn threads(&self) -> usize { self.threads }
	pub fn output(&self) -> &OutputOpt { &self.output } 
	pub fn set_dbsnp_file(&mut self, dbsnp_file: dbsnp::DBSnpFile) -> &mut Self { self.dbsnp_file = Some(dbsnp_file); self}
	pub fn dbsnp_file(&mut self) -> Option<dbsnp::DBSnpFile> { self.dbsnp_file.take() }
	pub fn set_selected_hash(&mut self, selected_hash: HashSet<String>) -> &mut Self { self.selected_hash = Some(selected_hash); self}
	pub fn selected_hash(&mut self) -> Option<HashSet<String>> { self.selected_hash.take() }
	pub fn synced_reader(&mut self) -> Option<BcfSrs> { self.synced_reader.take() }
}

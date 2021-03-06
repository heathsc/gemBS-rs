use std::collections::{HashMap, HashSet};
use std::io::{self, Error, ErrorKind};
use std::sync::RwLock;

use super::contig::*;

pub fn new_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

#[derive(Debug, Copy, Clone)]
pub enum IType { Auto, Bed, Vcf, Json }

pub struct Config {
	threads: usize,
	jobs: usize,
	output: Option<String>,
	description: RwLock<Option<String>>,
	input_type: IType,
	selected: HashSet<String>,
	maf_limit: Option<f64>,
	ctg_hash: ContigHash,
}

impl Config {
	pub fn new(threads: usize, jobs: usize, maf_limit: Option<f64>, output: Option<String>, description: Option<String>, input_type: IType,
		chrom_alias: Option<HashMap<String, String>>, selected: HashSet<String>) -> Self { 
		Self { threads, jobs, maf_limit, output, description: RwLock::new(description), input_type, selected, 
				ctg_hash: ContigHash::new(threads * 32, chrom_alias)}
	}
	pub fn threads(&self) -> usize { self.threads }
	pub fn jobs(&self) -> usize { self.jobs }
	pub fn maf_limit(&self) -> Option<f64> { self.maf_limit }
	pub fn input_type(&self) -> IType { self.input_type }
	pub fn output(&self) -> Option<&str> { self.output.as_deref()}
	pub fn selected<S: AsRef<str>> (&self, s: S) -> bool { self.selected.contains(s.as_ref())}
	pub fn ctg_hash(&self) -> &ContigHash {&self.ctg_hash}
	pub fn description(&self) -> Option<String> { (*self.description.read().unwrap()).as_ref().cloned() }
	// Set description if not aready set.  Returns true if description has been set.
	pub fn cond_set_description<S: AsRef<str>>(&self, desc: S) -> bool {
		let mut guard = self.description.write().unwrap(); 
		match &*guard {	
			Some(_) => false,
			None => {
				*guard = Some(desc.as_ref().to_string());	
				true
			}
		}
	}
}


pub enum DbInput {
	File(String),
	VcfContig(String, String, libc::c_int),	
}


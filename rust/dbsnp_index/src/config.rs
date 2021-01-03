use std::collections::{HashMap, HashSet};
use std::io::{self, Error, ErrorKind};

use super::contig::*;
use super::prefix::*;

pub fn new_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

#[derive(Copy, Clone)]
pub enum IType { Auto, Bed, Vcf, Json }

pub struct Config {
	threads: usize,
	output: Option<String>,
	description: Option<String>,
	input_type: IType,
	chrom_alias: HashMap<String, String>,
	selected: HashSet<String>,
	maf_limit: Option<f64>,
	ctg_hash: ContigHash,
	pref_hash: PrefixHash,
}

impl Config {
	pub fn new(threads: usize, maf_limit: Option<f64>, output: Option<String>, description: Option<String>, input_type: IType,
		chrom_alias: HashMap<String, String>, selected: HashSet<String>) -> Self { 
		Self { threads, maf_limit, output, description, input_type, chrom_alias, selected, 
				ctg_hash: ContigHash::new(threads * 32), pref_hash: PrefixHash::new() }
	}
	pub fn threads(&self) -> usize { self.threads }
	pub fn maf_limit(&self) -> Option<f64> { self.maf_limit }
	pub fn input_type(&self) -> IType { self.input_type }
	pub fn output(&self) -> Option<&String> { self.output.as_ref() }
	pub fn description(&self) -> Option<&String> { self.description.as_ref() }
	pub fn get_alias<S: AsRef<str>> (&self, s: S) -> Option<&String> { self.chrom_alias.get(s.as_ref())}
	pub fn selected<S: AsRef<str>> (&self, s: S) -> bool { self.selected.contains(s.as_ref())}
	pub fn ctg_hash(&self) -> &ContigHash {&self.ctg_hash}
	pub fn pref_hash(&self) -> &PrefixHash {&self.pref_hash}
}


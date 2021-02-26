use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};
use std::sync::{RwLock, Arc};

use crate::bbi::{Bbi, BbiBlockType};

pub fn new_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

pub struct VcfContig {
	name: Arc<Box<str>>,
	length: usize,
	out_ix: Option<usize>, // Output index (used for bbi files)
}

impl VcfContig {
	pub fn new<S: AsRef<str>>(name: S, length: usize) -> Self {
		Self { 
			name: Arc::new(name.as_ref().to_owned().into_boxed_str()), 
			length, 
			out_ix: None,
		 }
	}
	pub fn name(&self) -> &str { self.name.as_ref() }
	pub fn length(&self) -> usize { self.length }
	pub fn out_ix(&self) -> Option<usize> { self.out_ix }
}

#[derive(Debug,Copy, Clone)]
pub enum Mode { Combined, StrandSpecific }

#[derive(Debug,Copy, Clone)]
pub enum Select { Hom, Het }

#[derive(Debug,Clone)]
pub enum ConfVar {
	Bool(bool),
	Int(usize),
	Float(f64),
	String(Option<String>),
	Mode(Mode),
	Select(Select),
}

pub struct ConfHash {
	hash: HashMap<&'static str, ConfVar>,
	out_files: RwLock<Vec<(String, bool)>>,
	vcf_contigs: Vec<VcfContig>,
	vcf_contig_hash: HashMap<Arc<Box<str>>, usize>,
	bbi: RwLock<Option<Bbi>>,
	max_uncomp_size: RwLock<HashMap<BbiBlockType, usize>>,
}

impl ConfHash {
	pub fn new(hash: HashMap<&'static str, ConfVar>, vcf_contigs: Vec<VcfContig>) -> Self { 
		let vcf_contig_hash = vcf_contigs.iter().enumerate().fold(HashMap::new(), |mut h, (ix, ctg)| {h.insert(ctg.name.clone(), ix); h} );
		ConfHash {hash, vcf_contigs, vcf_contig_hash, out_files: RwLock::new(Vec::new()), bbi: RwLock::new(None), max_uncomp_size: RwLock::new(Default::default()) } 
	}
	pub fn vcf_contigs(&self) -> &[VcfContig] { &self.vcf_contigs }	
	pub fn contig_rid<S: AsRef<str>>(&self, ctg: S) -> Option<usize> { self.vcf_contig_hash.get(&(Box::<str>::from(ctg.as_ref()))).copied() }
	pub fn set_contig_out_ix(&mut self, rid: usize, out_ix: usize) { 
		assert!(rid < self.vcf_contigs.len());
		self.vcf_contigs[rid].out_ix = Some(out_ix);
	}
	pub fn get(&self,  key: &str) -> Option<&ConfVar> { self.hash.get(key) }
	pub fn set(&mut self, key: &'static str, val: ConfVar) { self.hash.insert(key, val); }

	pub fn get_bool(&self, key: &str) -> bool { 
		if let Some(ConfVar::Bool(x)) = self.get(key) { *x } else { panic!("Bool config var {} not set", key); }
	}
	
	pub fn get_int(&self, key: &str) -> usize { 
		if let Some(ConfVar::Int(x)) = self.get(key) { *x } else { panic!("Integer config var {} not set", key); }
	}

	pub fn get_float(&self, key: &str) -> f64 { 
		if let Some(ConfVar::Float(x)) = self.get(key) { *x } else { panic!("Flaot config var {} not set", key); }
	}

	pub fn get_str(&self, key: &str) -> Option<&str> { 
		if let Some(ConfVar::String(x)) = self.get(key) { x.as_deref() } else { panic!("String config var {} not set", key); }
	}
	pub fn get_select(&self, key: &str) -> Select { 
		if let Some(ConfVar::Select(x)) = self.get(key) { *x } else { panic!("Select config var {} not set", key); }
	}
	pub fn get_mode(&self, key: &str) -> Mode { 
		if let Some(ConfVar::Mode(x)) = self.get(key) { *x } else { panic!("Bool config var {} not set", key); }
	}
	pub fn n_out_files(&self) -> usize { self.out_files.read().unwrap().len() } 
	pub fn out_files(&self) -> Vec<(String, bool)> {
		let rf = self.out_files.read().unwrap();
		rf.iter().map(|s| s.to_owned()).collect()
	} 
	pub fn add_file<S: AsRef<str>>(&self, fname: S, tabix_flag: bool) {
		self.out_files.write().unwrap().push((fname.as_ref().to_owned(), tabix_flag));
	}
	pub fn set_bbi(&self, bbi: Bbi) { 
		trace!("set_bbi()");
		let mut p = self.bbi.write().unwrap();
		*p = Some(bbi);
		trace!("set_bbi() done");
	}
	pub fn drop_sender(&self) { 
		trace!("config drop_sender()");
		let mut p = self.bbi.write().unwrap();
		if let Some(bbi) = (*p).as_mut() { bbi.drop_sender() }
		trace!("drop_sender done()");
	}
	pub fn bbi(&self) -> &RwLock<Option<Bbi>> { &self.bbi }
	pub fn update_max_uncomp_size(&self, bbi_type: BbiBlockType, sz: usize) {
		let mut hash = self.max_uncomp_size.write().unwrap();
		let curr_val = hash.entry(bbi_type).or_insert(0);
		*curr_val = (*curr_val).max(sz);
	}
	pub fn max_uncomp_size(&self, bbi_type: BbiBlockType) -> Option<usize> { self.max_uncomp_size.read().unwrap().get(&bbi_type).copied() }
}



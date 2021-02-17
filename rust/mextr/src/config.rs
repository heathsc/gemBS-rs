use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};
use std::time::Duration;
use std::thread::sleep;
use std::sync::{RwLock, Arc};

use crate::bbi::{Bbi, bbi_zoom::ZoomData};

pub fn new_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

pub struct VcfContig {
	name: Arc<Box<str>>,
	length: usize,
	out_ix: Option<usize>, // Output index (used for bbi files)
	zoom_data: RwLock<Option<ZoomData>>,
}

impl VcfContig {
	pub fn new<S: AsRef<str>>(name: S, length: usize) -> Self {
		Self { 
			name: Arc::new(name.as_ref().to_owned().into_boxed_str()), 
			length, 
			out_ix: None,
			zoom_data: RwLock::new(None)
		 }
	}
	pub fn name(&self) -> &str { self.name.as_ref() }
	pub fn length(&self) -> usize { self.length }
	pub fn out_ix(&self) -> Option<usize> { self.out_ix }
	pub fn zoom_data(&self) -> &RwLock<Option<ZoomData>> { &self.zoom_data } 
	pub fn init_zoom_data(&self) {
		let mut p = self.zoom_data.write().unwrap();
		*p = Some(ZoomData::new(self.length));
	}
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
	out_files: RwLock<Vec<String>>,
	vcf_contigs: Vec<VcfContig>,
	vcf_contig_hash: HashMap<Arc<Box<str>>, usize>,
	bbi: RwLock<Option<Bbi>>,
	max_uncomp_size: RwLock<usize>,
}

impl ConfHash {
	pub fn new(hash: HashMap<&'static str, ConfVar>, vcf_contigs: Vec<VcfContig>) -> Self { 
		let vcf_contig_hash = vcf_contigs.iter().enumerate().fold(HashMap::new(), |mut h, (ix, ctg)| {h.insert(ctg.name.clone(), ix); h} );
		ConfHash {hash, vcf_contigs, vcf_contig_hash, out_files: RwLock::new(Vec::new()), bbi: RwLock::new(None), max_uncomp_size: RwLock::new(0) } 
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
	pub fn n_out_files(&self) -> usize {
		let d = Duration::from_millis(100);
		loop {
			if let Ok(rf) = self.out_files.try_read() {
				break rf.len();
			} else { sleep(d) }
		}
	} 
	pub fn out_files(&self) -> Vec<String> {
		let d = Duration::from_millis(100);
		loop {
			if let Ok(rf) = self.out_files.try_read() {
				let v: Vec<String> = rf.iter().map(|s| s.to_owned()).collect();
				break v;
			} else { sleep(d) }
		}
	} 
	pub fn add_file<S: AsRef<str>>(&self, fname: S) {
		let d = Duration::from_millis(100);
		loop {
			if let Ok(mut rf) = self.out_files.try_write() {
				rf.push(fname.as_ref().to_owned());
				break;
			} else { sleep(d) }
		}
	}
	pub fn set_bbi(&self, bbi: Bbi) { 
		let mut p = self.bbi.write().unwrap();
		*p = Some(bbi);
	}
	pub fn drop_sender(&self) { 
		let mut p = self.bbi.write().unwrap();
		if let Some(bbi) = (*p).as_mut() { bbi.drop_sender() }
	}
	pub fn bbi(&self) -> &RwLock<Option<Bbi>> { &self.bbi }
	pub fn update_max_uncomp_size(&self, sz: usize) {
		let mut mx = self.max_uncomp_size.write().unwrap();
		*mx = sz.max(*mx);
	}
}



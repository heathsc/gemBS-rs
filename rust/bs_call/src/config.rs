use std::collections::HashMap;
use std::str::FromStr;
use std::io;
use std::io::{Error, ErrorKind};

// use rust_htslib::htslib as hts;
use crate::htslib;
use crate::defs::{CtgInfo, CtgRegion};

pub fn new_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

#[derive(Copy, Clone)]
pub struct OType(u32);

impl OType {
	pub fn new(x: u32) -> Self {
		if x == htslib::FT_UNKN || x > htslib::FT_BCF_GZ { panic!("Illegal filetype {}", x); }
		OType(x)
	}
	pub fn is_compressed(&self) -> bool { (self.0 & htslib::FT_GZ) != 0 }	
	pub fn is_vcf(&self) -> bool { (self.0 & htslib::FT_VCF) != 0 }	
	pub fn is_bcf(&self) -> bool { (self.0 & htslib::FT_BCF) != 0 }	
	pub fn eq_u32(&self, x: u32) -> bool { self.0 == x }
}

impl FromStr for OType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "b" => Ok(OType(htslib::FT_BCF_GZ)),
            "u" => Ok(OType(htslib::FT_BCF)),
            "z" => Ok(OType(htslib::FT_VCF_GZ)),
            "v" => Ok(OType(htslib::FT_VCF)),
            _ => Err("no match"),
        }
    }
}

#[derive(Clone)]
pub enum ConfVar {
	Bool(bool),
	Int(usize),
	Float(f64),
	String(Option<String>),
	OType(OType),
}

pub struct ConfHash {
	hash: HashMap<&'static str, ConfVar>,
}

impl ConfHash {
	pub fn new(hash: HashMap<&'static str, ConfVar>) -> Self { ConfHash {hash} }
	pub fn get(&self,  key: &str) -> Option<&ConfVar> { self.hash.get(key) }
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

	pub fn get_otype(&self) -> OType {
		if let Some(ConfVar::OType(x)) = self.get("output_type") { *x } else { panic!("Config var output_type not set"); }		
	}
}

pub struct BsCallConfig {
	conf_hash: ConfHash,
	sam_input: htslib::HtsFile,
	sam_index: htslib::HtsIndex,
	sam_header: htslib::SamHeader,
	ref_index: htslib::Faidx,
	contigs: Vec<CtgInfo>,
	contig_regions: Vec<CtgRegion>,
}

impl BsCallConfig {
	pub fn new(conf_hash: ConfHash, sam_input: htslib::HtsFile, sam_index: htslib::HtsIndex, sam_header: htslib::SamHeader, ref_index: htslib::Faidx) -> Self { 
		Self{conf_hash, sam_input, sam_index, sam_header, ref_index, contigs: Vec::new(), contig_regions: Vec::new()} 
	}
	
	pub fn set_conf(&mut self, key: &'static str, var: ConfVar) -> Option<ConfVar> {
		self.conf_hash.hash.insert(key, var)
	}
	
	pub fn get_conf(&self, key: &str) -> Option<&ConfVar> { self.conf_hash.get(key) }
	pub fn get_conf_bool(&self, key: &str) -> bool { self.conf_hash.get_bool(key) }
	pub fn get_conf_int(&self, key: &str) -> usize { self.conf_hash.get_int(key) }
	pub fn get_conf_float(&self, key: &str) -> f64 { self.conf_hash.get_float(key) }
	pub fn get_conf_str(&self, key: &str) -> Option<&str> { self.conf_hash.get_str(key) }
	pub fn get_conf_otype(&self) -> OType { self.conf_hash.get_otype() }
	
	pub fn ref_index(&self) -> &htslib::Faidx { &self.ref_index }	
	pub fn sam_input(&self) -> &htslib::HtsFile { &self.sam_input }
	pub fn sam_index(&self) -> &htslib::HtsIndex { &self.sam_index }
	pub fn sam_header(&self) -> &htslib::SamHeader { &self.sam_header }
	
	pub fn add_contigs(&mut self, ctgs: &mut[CtgInfo]) { self.contigs.extend_from_slice(ctgs); }
	pub fn add_contig_regions(&mut self, creg: &mut[CtgRegion]) { self.contig_regions.extend_from_slice(creg); }

	pub fn ctg_in_header(&self, idx: usize) -> bool { self.contigs[idx].in_header() }
	pub fn n_ctgs(&self) -> usize { self.sam_header.nref() }
	pub fn ctg_name(&self, idx: usize) -> &str { self.sam_header.tid2name(idx) }
	pub fn ctg_len(&self, idx: usize) -> usize { self.sam_header.tid2len(idx) }
	pub fn ctg_id<S: AsRef<str>>(&self, seq: S) -> Option<usize> { self.sam_header.name2tid(seq) }
}

use std::collections::HashMap;
use std::str::FromStr;
use std::{io, fmt};
use std::io::{Error, ErrorKind};

// use rust_htslib::htslib as hts;
use crate::htslib;
use crate::defs::{CtgRegion, CtgInfo};

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

impl fmt::Display for OType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", match self.0 {
			htslib::FT_BCF_GZ => "wb", 
			htslib::FT_BCF => "wbu",
			htslib::FT_VCF_GZ => "wz",
			_ => "w",
		})
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
	pub conf_hash: ConfHash,
	pub sam_input: htslib::SamFile,
	pub ref_index: htslib::Faidx,
	pub vcf_output: htslib::VcfFile,
	pub contigs: Vec<CtgInfo>,
	pub regions: Vec<CtgRegion>,
}

impl BsCallConfig {
	pub fn new(conf_hash: ConfHash, sam_input: htslib::SamFile, vcf_output: htslib::VcfFile, ref_index: htslib::Faidx) -> Self { 
		Self{conf_hash, sam_input, vcf_output, ref_index, contigs: Vec::new(), regions: Vec::new()} 
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
	pub fn sam_input(&self) -> &htslib::SamFile { &self.sam_input }
	pub fn vcf_output(&mut self) -> &mut htslib::VcfFile { &mut self.vcf_output }
	
	pub fn add_contigs(&mut self, ctgs: &mut[CtgInfo]) { self.contigs.extend_from_slice(ctgs); }
	pub fn add_regions(&mut self, regions: &mut[CtgRegion]) { self.regions.extend_from_slice(regions); }
	pub fn ctg_in_header(&self, idx: usize) -> bool { self.contigs[idx].in_header() }

	pub fn ctg_vcf_id(&self, idx: usize) -> Option<usize> { self.contigs[idx].vcf_id() }
	pub fn ctg_ref_id(&self, idx: usize) -> Option<usize> { self.contigs[idx].ref_id() }
	pub fn n_ctgs(&self) -> usize { self.sam_input.nref() }
	pub fn ctg_name(&self, idx: usize) -> &str { self.sam_input.tid2name(idx) }
	pub fn ctg_len(&self, idx: usize) -> usize { self.sam_input.tid2len(idx) }
	pub fn ctg_id<S: AsRef<str>>(&self, seq: S) -> Option<usize> { self.sam_input.name2tid(seq) }
}

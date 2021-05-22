use std::collections::HashMap;
use std::str::FromStr;
use std::{io, fmt};
use std::io::{Error, ErrorKind};

use crate::htslib;
use crate::defs::{CtgRegion, CtgInfo};

use crate::dbsnp;

pub fn new_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

#[derive(Debug, Copy, Clone)]
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

#[derive(Debug,Clone)]
pub enum ConfVar {
	Bool(bool),
	Int(usize),
	Float(f64),
	String(Option<String>),
	OType(OType),
}

#[derive(Clone)]
pub struct ConfHash {
	hash: HashMap<&'static str, ConfVar>,
}

impl ConfHash {
	pub fn new(hash: HashMap<&'static str, ConfVar>) -> Self { ConfHash {hash} }
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

	pub fn get_otype(&self) -> OType {
		if let Some(ConfVar::OType(x)) = self.get("output_type") { *x } else { panic!("Config var output_type not set"); }		
	}
}

pub struct BsCallConfig {
	pub conf_hash: ConfHash,
	pub contigs: Vec<CtgInfo>,
	pub regions: Vec<CtgRegion>,
}

impl BsCallConfig {
	pub fn new(conf_hash: ConfHash, contigs: Vec<CtgInfo>, regions: Vec<CtgRegion>) -> Self { 
		Self{conf_hash, contigs, regions} 
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
	pub fn add_contigs(&mut self, ctgs: &mut[CtgInfo]) { self.contigs.extend_from_slice(ctgs); }
	pub fn ctg_in_header(&self, idx: usize) -> bool { self.contigs[idx].in_header() }
	pub fn ctg_vcf_id(&self, idx: usize) -> Option<usize> { self.contigs[idx].vcf_id() }
	pub fn ctg_ref_id(&self, idx: usize) -> Option<usize> { self.contigs[idx].ref_id() }
	pub fn ctg_name(&self, idx: usize) -> &str { self.contigs[idx].name() }
}

pub struct BsCallFiles {
	pub sam_input: Option<htslib::SamFile>,
	pub ref_index: Option<htslib::Faidx>,
	pub vcf_output: Option<htslib::VcfFile>,
	pub dbsnp_index: Option<dbsnp::DBSnpIndex>,	
}

impl BsCallFiles {
	pub fn new(sam_input: htslib::SamFile, vcf_output: htslib::VcfFile, ref_index: htslib::Faidx, dbsnp_index: Option<dbsnp::DBSnpIndex>) -> Self { 
		Self{sam_input: Some(sam_input), vcf_output: Some(vcf_output), ref_index: Some(ref_index), dbsnp_index} 
	}	
}

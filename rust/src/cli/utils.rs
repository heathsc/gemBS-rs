use clap::ArgMatches;
use std::str::FromStr;
use std::collections::HashMap;
use crate::config::GemBS;
use lazy_static::lazy_static;
use crate::common::defs::{Section, DataValue, VarType, FileType, JobLen, MemSize};

pub fn from_arg_matches<T: FromStr>(m: &ArgMatches, option: &str) -> Option<T> {
	match m.value_of(option) {
		None => None,
		Some(s) => {
			match <T>::from_str(s) {
				Ok(i) => Some(i),
				_ => {
					error!("Invalid value '{}' for option '{}'", s, option);
					None
				},
			}
		}
	}
}

pub fn get_option(m: &ArgMatches, opt: &str, tt: VarType) -> Option<DataValue> {
	match tt {
		VarType::Int => m.value_of(opt).and_then(|x| <isize>::from_str(x).ok().map(DataValue::Int)),
		VarType::Bool => if m.is_present(opt) { Some(DataValue::Bool(true)) } else { None },
		VarType::Float => m.value_of(opt).and_then(|x| <f64>::from_str(x).ok().map(DataValue::Float)),
		VarType::String => m.value_of(opt).map(|x| DataValue::String(x.to_owned())),
		VarType::FileType => m.value_of(opt).and_then(|x| <FileType>::from_str(x).ok().map(DataValue::FileType)),
		VarType::JobLen => m.value_of(opt).and_then(|x| <JobLen>::from_str(x).ok().map(DataValue::JobLen)),
		VarType::MemSize => m.value_of(opt).and_then(|x| <MemSize>::from_str(x).ok().map(DataValue::MemSize)),
		VarType::FloatVec => m.values_of(opt).map(|v| {			
			let vec:Vec<_> = v.map(|x| <f64>::from_str(x).ok().unwrap()).collect();
			DataValue::FloatVec(vec)
		}),
		VarType::StringVec => m.values_of(opt).map(|v| {
			let args: Vec<String> = v.map(|x| x.to_owned()).collect();
			DataValue::StringVec(args)
		}),

		_ => None,
	}
}

pub	fn handle_options(m: &ArgMatches, gem_bs: &mut GemBS, section: Section) -> HashMap<&'static str, DataValue> {
	let mut options = HashMap::new();
	for (opt, val) in OPT_ASSOC.iter() {
		match val {
			OptionType::Global(s, vt) => {
				if let Some(x) = get_option(m, opt, *vt) { 
					options.insert(*opt, x.clone()); 
					gem_bs.set_config(section, s, x); 
				}
			},
			OptionType::Local(vt) => {
				if let Some(x) = get_option(m, opt, *vt) { 
					debug!("Setting local option {} to {:?}", opt, x);
					options.insert(*opt, x); 
				}
			},
			OptionType::Special(s, vt) => {
				if let Some(x) = get_option(m, opt, *vt) { 
					debug!("Setting special option {} to {:?}", opt, x);
					options.insert(s, x); 
				}
			},
		}		
	}
	options
}

#[derive(Debug, Copy, Clone)]
pub enum OptionType {
	Global(&'static str, VarType),
	Local(VarType),
	Special(&'static str, VarType),
}

lazy_static! {
    pub static ref OPT_ASSOC: Vec<(&'static str, OptionType)> = {
        let mut m = Vec::new();
        m.push(("threads", OptionType::Global("threads", VarType::Int)));
        m.push(("map_threads", OptionType::Global("map_threads", VarType::Int)));
        m.push(("merge_threads", OptionType::Global("merge_threads", VarType::Int)));
        m.push(("sort_threads", OptionType::Global("sort_threads", VarType::Int)));
        m.push(("call_threads", OptionType::Global("call_threads", VarType::Int)));
        m.push(("cores", OptionType::Global("cores", VarType::Int)));
        m.push(("time", OptionType::Global("time", VarType::JobLen)));
        m.push(("memory", OptionType::Global("memory", VarType::MemSize)));
        m.push(("sort_memory", OptionType::Global("sort_memory", VarType::MemSize)));
        m.push(("tmp_dir", OptionType::Global("tmp_dir", VarType::String)));
        m.push(("underconv_seq", OptionType::Global("underconversion_sequence", VarType::String)));
        m.push(("overconv_seq", OptionType::Global("overconversion_sequence", VarType::String)));
        m.push(("reverse", OptionType::Global("reverse_conversion", VarType::Bool)));
        m.push(("non_stranded", OptionType::Global("non_stranded", VarType::Bool)));
        m.push(("benchmark_mode", OptionType::Global("benchmark_mode", VarType::Bool)));
		m.push(("jobs", OptionType::Global("jobs", VarType::Int)));
        m.push(("non_bs", OptionType::Local(VarType::Bool)));
        m.push(("bs", OptionType::Local(VarType::Bool)));
      	m.push(("no_merge", OptionType::Special("_no_merge", VarType::Bool)));
      	m.push(("no_md5", OptionType::Special("_no_md5", VarType::Bool)));
      	m.push(("no_index", OptionType::Special("_no_index", VarType::Bool)));
      	m.push(("merge", OptionType::Local(VarType::Bool)));
      	m.push(("remove", OptionType::Local(VarType::Bool)));
      	m.push(("paired", OptionType::Local(VarType::Bool)));
     	m.push(("file_type", OptionType::Local(VarType::FileType)));
		m.push(("sample", OptionType::Special("_sample", VarType::StringVec)));
		m.push(("barcode", OptionType::Special("_barcode", VarType::StringVec)));
		m.push(("dataset", OptionType::Special("_dataset", VarType::StringVec)));
      	m.push(("list_pools", OptionType::Special("_list_pools", VarType::Int)));
      	m.push(("pool", OptionType::Special("_pool", VarType::StringVec)));
      	m.push(("haploid", OptionType::Global("haploid", VarType::Bool)));
      	m.push(("keep_duplicates", OptionType::Global("keep_duplicates", VarType::Bool)));
      	m.push(("ignore_duplicate_flag", OptionType::Global("ignore_duplicate_flag", VarType::Bool)));
     	m.push(("keep_unmatched", OptionType::Global("keep_improper_pairs", VarType::Bool)));
      	m.push(("mapq_threshold", OptionType::Global("mapq_threshold", VarType::Int)));
      	m.push(("qual_threshold", OptionType::Global("qual_threshold", VarType::Int)));
      	m.push(("phred_threshold", OptionType::Global("phred_threshold", VarType::Int)));
      	m.push(("left_trim", OptionType::Global("left_trim", VarType::Int)));
      	m.push(("right_trim", OptionType::Global("right_trim", VarType::Int)));
      	m.push(("conversion", OptionType::Global("conversion", VarType::FloatVec)));
      	m.push(("auto_conversion", OptionType::Global("auto_conversion", VarType::Bool)));
    	m.push(("ref_bias", OptionType::Global("reference_bias", VarType::Float)));
      	m.push(("strand_specific", OptionType::Global("strand_specific", VarType::Bool)));
      	m.push(("bigwig_strand_specific", OptionType::Global("bigwig_strand_specific", VarType::Bool)));
      	m.push(("min_inform", OptionType::Global("min_inform", VarType::Int)));
      	m.push(("min_nc", OptionType::Global("min_nc", VarType::Int)));
      	m.push(("allow_het", OptionType::Global("allow_het", VarType::Bool)));
      	m.push(("ref_bias", OptionType::Global("reference_bias", VarType::Float)));
     	m.push(("cpg", OptionType::Global("make_cpg", VarType::Bool)));
     	m.push(("non_cpg", OptionType::Global("make_non_cpg", VarType::Bool)));
     	m.push(("bed_methyl", OptionType::Global("make_bedmethyl", VarType::Bool)));
     	m.push(("snps", OptionType::Global("make_snps", VarType::Bool)));
    	m.push(("snp_list", OptionType::Global("snp_list", VarType::String)));
    	m.push(("snp_db", OptionType::Global("dbsnp_index", VarType::String)));
 	  	m.push(("sampling", OptionType::Global("sampling_rate", VarType::Int)));
	  	m.push(("make_bs_index", OptionType::Local(VarType::Bool)));
	  	m.push(("make_dbsnp_index", OptionType::Local(VarType::Bool)));
	  	m.push(("dbsnp_files", OptionType::Global("dbsnp_files", VarType::StringVec)));
      	m.push(("dry_run", OptionType::Special("_dry_run", VarType::Bool)));
      	m.push(("json", OptionType::Special("_json", VarType::String)));
	  	m.push(("project", OptionType::Global("project", VarType::String)));
	  	m.push(("report_dir", OptionType::Global("report_dir", VarType::String)));
        m
    };
}

#[derive(Debug)]
pub struct LogLevel {
	level: usize,
}

impl FromStr for LogLevel {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(LogLevel{level: 0}),
            "warn" => Ok(LogLevel{level: 1}),
            "info" => Ok(LogLevel{level: 2}),
            "debug" => Ok(LogLevel{level: 3}),
            "trace" => Ok(LogLevel{level: 4}),
            "none" => Ok(LogLevel{level: 5}),
            _ => Err("no match"),
        }
    }
}

impl LogLevel {
	pub fn is_none(&self) -> bool {
		self.level > 4 
	}
	pub fn get_level(&self) -> usize {
		if self.level > 4 { 0 } else { self.level }
	}
	pub fn new(x: usize) -> Self {
		LogLevel{level: x}
	}
}

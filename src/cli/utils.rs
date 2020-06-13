use clap::ArgMatches;
use std::str::FromStr;
use std::collections::HashMap;
use crate::config::GemBS;
use lazy_static::lazy_static;
use crate::common::defs::{Section, DataValue, VarType, FileType};

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
		_ => None,
	}
}

pub	fn handle_options(m: &ArgMatches, gem_bs: &mut GemBS, section: Section, options: &mut HashMap<&'static str, DataValue>) {
	for (opt, val) in OPT_ASSOC.iter() {
		match val {
			OptionType::Global(s, vt) => {
				if let Some(x) = get_option(m, opt, *vt) { gem_bs.set_config(section, s, x); }
			},
			OptionType::Local(vt) => {
				if let Some(x) = get_option(m, opt, *vt) { 
					debug!("Setting local option {} to {:?}", opt, x);
					options.insert(opt, x); 
				}
			},
		}		
	}
}

#[derive(Debug, Copy, Clone)]
pub enum OptionType {
	Global(&'static str, VarType),
	Local(VarType),
}

lazy_static! {
    pub static ref OPT_ASSOC: Vec<(&'static str, OptionType)> = {
        let mut m = Vec::new();
        m.push(("threads", OptionType::Global("threads", VarType::Int)));
        m.push(("map_threads", OptionType::Global("map_threads", VarType::Int)));
        m.push(("merge_threads", OptionType::Global("merge_threads", VarType::Int)));
        m.push(("sort_threads", OptionType::Global("sort_threads", VarType::Int)));
        m.push(("tmp_dir", OptionType::Global("tmp_dir", VarType::String)));
        m.push(("underconv_seq", OptionType::Global("underconversion_sequence", VarType::String)));
        m.push(("overconv_seq", OptionType::Global("overconversion_sequence", VarType::String)));
        m.push(("reverse", OptionType::Global("reverse_conversion", VarType::Bool)));
        m.push(("non_stranded", OptionType::Global("non_stranded", VarType::Bool)));
        m.push(("benchmark_mode", OptionType::Global("benchmark_mode", VarType::Bool)));
		m.push(("jobs", OptionType::Global("jobs", VarType::Int)));
        m.push(("non_bs", OptionType::Local(VarType::Bool)));
        m.push(("bs", OptionType::Local(VarType::Bool)));
      	m.push(("no_merge", OptionType::Local(VarType::Bool)));
      	m.push(("dry_run", OptionType::Local(VarType::Bool)));
      	m.push(("json", OptionType::Local(VarType::Bool)));
      	m.push(("remove", OptionType::Local(VarType::Bool)));
      	m.push(("paired", OptionType::Local(VarType::Bool)));
     	m.push(("file_type", OptionType::Local(VarType::FileType)));
		m.push(("sample", OptionType::Local(VarType::String)));
		m.push(("barcode", OptionType::Local(VarType::String)));
		m.push(("dataset", OptionType::Local(VarType::String)));
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

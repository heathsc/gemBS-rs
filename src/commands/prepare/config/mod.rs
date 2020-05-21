use std::collections::HashMap;
use std::io::BufRead;

use crate::common::defs::{Section, VarType};
	
pub mod lex;
use lex::{Lexer, LexToken};

#[derive(Debug)]
pub enum RawVar {
	StringVar(Option<String>),
	BoolVar(Option<bool>), 
	IntVar(Option<isize>),	
}

#[derive(Debug)]
pub struct PrepConfigVar {
	var: RawVar,
	section: Section,
	known: bool,
	used: bool,
}

#[derive(Debug)]
pub struct KnownVar {
//	name: &'static str,
	vtype: VarType,
	sections: Vec<Section>
}

impl KnownVar {
	pub fn new(vtype: VarType, sections: Vec<Section>) -> Self {
		KnownVar {vtype, sections}
	}
}

#[derive(Debug)]
struct KnownVarList {
//	known_var: Vec<KnownVar>
	known_var: HashMap<&'static str, KnownVar>
}

impl KnownVarList {
	fn new() -> Self {
		KnownVarList{known_var: HashMap::new()}	
	}	
	fn add_known_var(&mut self, name: &'static str, vtype: VarType, sections: Vec<Section>) {
		self.known_var.insert(name, KnownVar::new(vtype, sections));
	}
	fn check_vtype(&self, name: &str, section: Section) -> Option<VarType> {
	 	match self.known_var.get(name) {
			Some(v) => if v.sections.contains(&section) { Some(v.vtype) } else { None }
			None => None,    
		}			
	}
}

fn make_known_var_list() -> KnownVarList {
	let mut kv_list = KnownVarList::new();
	kv_list.add_known_var("index", VarType::StringVar, vec!(Section::Index));
	kv_list.add_known_var("nonbs_index", VarType::StringVar, vec!(Section::Index));
	kv_list.add_known_var("index_dir", VarType::StringVar, vec!(Section::Index));
	kv_list.add_known_var("reference", VarType::StringVar, vec!(Section::Index));
	kv_list.add_known_var("extra_reference", VarType::StringVar, vec!(Section::Index));
	kv_list.add_known_var("reference_basename", VarType::StringVar, vec!(Section::Index));
	kv_list.add_known_var("contig_sizes", VarType::StringVar, vec!(Section::Index));
	kv_list.add_known_var("dbsnp_files", VarType::StringVar, vec!(Section::Index));
	kv_list.add_known_var("dbsnp_index", VarType::StringVar, vec!(Section::Index));
	kv_list.add_known_var("sampling_rate", VarType::IntVar, vec!(Section::Index));
	kv_list.add_known_var("populate_cache", VarType::BoolVar, vec!(Section::Index));
	kv_list.add_known_var("threads", VarType::IntVar, vec!(Section::Index, Section::Mapping, Section::Calling, Section::Extract, Section::Report));
	kv_list.add_known_var("merge_threads", VarType::IntVar, vec!(Section::Mapping, Section::Calling));
	kv_list.add_known_var("map_threads", VarType::IntVar, vec!(Section::Mapping));
	kv_list.add_known_var("sort_threads", VarType::IntVar, vec!(Section::Mapping));
	kv_list.add_known_var("sort_memory", VarType::StringVar, vec!(Section::Mapping));
	kv_list.add_known_var("non_stranded", VarType::BoolVar, vec!(Section::Mapping));
	kv_list.add_known_var("reverse_conversion", VarType::BoolVar, vec!(Section::Mapping));
	kv_list.add_known_var("remove_individual_bams", VarType::BoolVar, vec!(Section::Mapping));
	kv_list.add_known_var("underconversion_sequence", VarType::StringVar, vec!(Section::Mapping));
	kv_list.add_known_var("overconversion_sequence", VarType::StringVar, vec!(Section::Mapping));
	kv_list.add_known_var("bam_dir", VarType::StringVar, vec!(Section::Mapping));
	kv_list.add_known_var("sequence_dir", VarType::StringVar, vec!(Section::Mapping));
	kv_list.add_known_var("benchmark_mode", VarType::BoolVar, vec!(Section::Mapping, Section::Calling));
	kv_list.add_known_var("make_cram", VarType::BoolVar, vec!(Section::Mapping));
	kv_list.add_known_var("jobs", VarType::IntVar, vec!(Section::Calling, Section::Extract));
	kv_list.add_known_var("bcf_dir", VarType::StringVar, vec!(Section::Calling));
	kv_list.add_known_var("mapq_threshold", VarType::IntVar, vec!(Section::Calling));
	kv_list.add_known_var("qual_threshold", VarType::IntVar, vec!(Section::Calling));
	kv_list.add_known_var("left_trim", VarType::IntVar, vec!(Section::Calling));
	kv_list.add_known_var("right_trim", VarType::IntVar, vec!(Section::Calling));
	kv_list.add_known_var("species", VarType::StringVar, vec!(Section::Calling));
	kv_list.add_known_var("keep_duplicates", VarType::BoolVar, vec!(Section::Calling));
	kv_list.add_known_var("keep_improper_pairs", VarType::BoolVar, vec!(Section::Calling));
	kv_list.add_known_var("call_threads", VarType::IntVar, vec!(Section::Calling));
	kv_list.add_known_var("remove_individual_bcfs", VarType::BoolVar, vec!(Section::Calling));
	kv_list.add_known_var("haploid", VarType::BoolVar, vec!(Section::Calling));
	kv_list.add_known_var("reference_bias", VarType::FloatVar, vec!(Section::Calling, Section::Extract));
	kv_list.add_known_var("over_conversion_rate", VarType::FloatVar, vec!(Section::Calling));
	kv_list.add_known_var("under_conversion_rate", VarType::FloatVar, vec!(Section::Calling));
	kv_list.add_known_var("conversion", VarType::StringVar, vec!(Section::Calling));
	kv_list.add_known_var("contig_list", VarType::StringVar, vec!(Section::Calling));
	kv_list.add_known_var("contig_pool_limit", VarType::IntVar, vec!(Section::Calling));
	kv_list.add_known_var("extract_dir", VarType::StringVar, vec!(Section::Extract));
	kv_list.add_known_var("snp_list", VarType::StringVar, vec!(Section::Extract));
	kv_list.add_known_var("snp_db", VarType::StringVar, vec!(Section::Extract));
	kv_list.add_known_var("allow_het", VarType::BoolVar, vec!(Section::Extract));
	kv_list.add_known_var("phred_threshold", VarType::IntVar, vec!(Section::Extract));
	kv_list.add_known_var("min_inform", VarType::IntVar, vec!(Section::Extract));
	kv_list.add_known_var("extract_threads", VarType::IntVar, vec!(Section::Extract));
	kv_list.add_known_var("min_nc", VarType::IntVar, vec!(Section::Extract));
	kv_list.add_known_var("make_cpg", VarType::BoolVar, vec!(Section::Extract));
	kv_list.add_known_var("make_noncpg", VarType::BoolVar, vec!(Section::Extract));
	kv_list.add_known_var("make_bedmethyl", VarType::BoolVar, vec!(Section::Extract));
	kv_list.add_known_var("make_snps", VarType::BoolVar, vec!(Section::Extract));
	kv_list.add_known_var("bigwig_strand-specific", VarType::BoolVar, vec!(Section::Extract));
	kv_list.add_known_var("strand-specific", VarType::BoolVar, vec!(Section::Extract));
	kv_list.add_known_var("project", VarType::StringVar, vec!(Section::Report));
	kv_list.add_known_var("report_dir", VarType::StringVar, vec!(Section::Report));
	kv_list
}

#[derive(Debug, PartialEq)]
enum ParserState { WaitingForName, WaitingForEquals, WaitingForValue, AfterValue, End }	

pub struct PrepConfig {
	kv_list: KnownVarList,
	var: HashMap<&'static str, Vec<PrepConfigVar>>,
	lexer: Lexer,
}

impl PrepConfig {
	pub fn new() -> Self {
		PrepConfig{
			kv_list: make_known_var_list(), 
			var: HashMap::new(), 
			lexer: Lexer::new()
		}
	}
	pub fn check_vtype(&self, name: &str, section: Section) -> Option<VarType> {
		self.kv_list.check_vtype(name, section)		
	}
	pub fn start_parse(&mut self, name: &str) -> Result<(), String> {
		self.lexer.init_lexer(name)
	}

	fn handle_name(&mut self, tok: LexToken, ostr: Option<String>) -> Result<ParserState, String> {
		match tok {
			LexToken::End => Ok(ParserState::End),
			LexToken::Name => Ok(ParserState::WaitingForEquals),
			_ => Err("Unexpected token - waiting for variable name".to_string()),
		}
	}	
	fn handle_equals(&mut self, tok: LexToken) -> Result <ParserState, String> {
		match tok {
			LexToken::Equals => Ok(ParserState::WaitingForValue),
			_ => Err("Unexpected token - waiting for variable name".to_string()),
		}
	}
	fn handle_value(&mut self, tok: LexToken, ostr: Option<String>) -> Result <ParserState, String> {
		match tok {
			LexToken::Value => Ok(ParserState::WaitingForValue),
			_ => Err("Unexpected token - waiting for variable name".to_string()),
		}
	}
	fn handle_after_value(&mut self, tok: LexToken, ostr: Option<String>) -> ParserState {
		return ParserState::WaitingForName;
	}
	pub fn parse(&mut self) -> Result<(), String> {
		let mut state = ParserState::WaitingForValue; 
		loop {
			let s = self.lexer.get_token()?;
			match s.0 {
				LexToken::End => break,
				_ => println!("Got {:?} {:?}", s.0, s.1),
			}
/*
			match state {
				ParserState::WaitingForName => {
					state = self.handle_name(s.0, s.1)?;
				},
				ParserState::WaitingForEquals => {
					state = self.handle_equals(s.0)?;
					
				},
				ParserState::WaitingForValue => {
					state = self.handle_value(s.0, s.1)?;
					
				},
				ParserState::AfterValue => {
					state = self.handle_after_value(s.0, s.1);
					
				}
				_ => (),
			}
			if state == ParserState::End { break; } */
		}
		Ok(())
	}	
}


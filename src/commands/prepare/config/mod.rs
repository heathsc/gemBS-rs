use std::collections::HashMap;
use std::str::FromStr;
use std::env;

use crate::common::defs::{Section, VarType};
	
pub mod lex;
mod find_var;
use lex::{Lexer, LexToken};
use find_var::{find_var, Segment};

#[derive(Debug, Clone)]
pub enum RawVar {
	StringVar(String),
	StringVec(Vec<String>),
	BoolVar(bool), 
	IntVar(isize),	
	FloatVar(f64),
	FloatVec(Vec<f64>),
}

#[derive(Debug, Clone)]
pub struct PrepConfigVar {
	var: RawVar,
	vtype:VarType,
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
	known_var: HashMap<&'static str, KnownVar>
}

impl KnownVarList {
	fn new() -> Self {
		KnownVarList{known_var: HashMap::new()}	
	}	
	fn add_known_var(&mut self, name: &'static str, vtype: VarType, sections: Vec<Section>) {
		let mut v = sections;
		v.push(Section::Default);
		self.known_var.insert(name, KnownVar::new(vtype, v));
	}
	fn check_vtype(&self, name: &str, section: Section) -> Option<VarType> {
		let tstr = name.to_lowercase();
	 	match self.known_var.get(&tstr.as_str()) {
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
	kv_list.add_known_var("conversion", VarType::FloatVec, vec!(Section::Calling));
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
	kv_list.add_known_var("bigwig_strand_specific", VarType::BoolVar, vec!(Section::Extract));
	kv_list.add_known_var("strand_specific", VarType::BoolVar, vec!(Section::Extract));
	kv_list.add_known_var("project", VarType::StringVar, vec!(Section::Report));
	kv_list.add_known_var("report_dir", VarType::StringVar, vec!(Section::Report));
	kv_list
}

#[derive(Debug)]
enum ParserState { 
	WaitingForName, 
	WaitingForValue(String), 
	AfterValue((String, Section, PrepConfigVar)), 
	End 
}	

pub struct PrepConfig {
	kv_list: KnownVarList,
	var: HashMap<String, Vec<PrepConfigVar>>,
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
	pub fn get(&self, name: &str, section: Section) -> Option<&PrepConfigVar> {
		if name.is_empty() { return None; }
		match self.var.get(&name.to_lowercase()) {			
			Some(v) => {
				let mut default_var = None;
				let mut var = None;
				for pv in v.iter() {
					if pv.section == section {
						var = Some(pv);
					} else if pv.section == Section::Default {
						default_var = Some(pv);
					}
				}
				if var.is_some() { var } else { default_var }
			},
			None => None,	
		}
	}
	fn handle_name(&self, tok: LexToken) -> Result<ParserState, String> {
		match tok {
			LexToken::Name(name) => Ok(ParserState::WaitingForValue(name.to_lowercase())),
			LexToken::End => Ok(ParserState::End),
			_ => Err("Unexpected token - waiting for variable name".to_string()),
		}
	}
		
	fn handle_value(&mut self, tok: LexToken, name: String) -> Result <ParserState, String> {
		match tok {
			LexToken::Value(val_str) => {
				let section = if let Some(s) = self.lexer.get_section() { s } else { return Err("Internal error - no section".to_string()) };
				let vt = self.check_vtype(&name, section);
				let known = vt.is_some();
				let vt = vt.unwrap_or(VarType::StringVar);
//		let mut rv = self.get_raw_var_from_string(val_str, vt, &name)?;

				// We initially keep everything as a string until after finishing parsing the config file(s)
				// To allow interpolation to work as expected
				let rv = if let VarType::FloatVec = vt {
					RawVar::StringVec(vec!(val_str;1))
				} else {
					RawVar::StringVar(val_str)
				};
				let pvar = PrepConfigVar{var: rv, vtype: vt, section, known, used: false};
				Ok(ParserState::AfterValue((name, section, pvar)))
			},
			_ => Err(format!("Unexpected token - waiting for value after variable {}", name)),
		}
	}
	fn handle_next_value(&mut self, val_str: String, mut pvar: PrepConfigVar, name: String, section: Section) -> Result<ParserState, String> {
		let mut vector = match pvar.var {
			RawVar::StringVec(x) => x,
			_ => return Err("Internal error in handle_next_value()".to_string()),
		};
		vector.push(val_str);
		pvar.var = RawVar::StringVec(vector);
		Ok(ParserState::AfterValue((name, section, pvar)))
	}	
	fn handle_after_value(&mut self, tok: LexToken, x: (String, Section, PrepConfigVar)) -> Result<ParserState, String> {
		let name = x.0;
		let section = x.1;
		let pvar = x.2;
		let vector = if let RawVar::StringVec(_) = pvar.var { true } else { false };
		match tok {
			LexToken::Name(_) => {
				self.handle_assignment(name, section, pvar)?;
				self.handle_name(tok)
			},
			LexToken::Value(st) if vector => self.handle_next_value(st, pvar, name, section),
			LexToken::End => {
				self.handle_assignment(name, section, pvar)?;
				Ok(ParserState::End)
			},
			_ => Err(format!("Unexpected token - waiting for variable name or value after variable {}", name)),
		}
	} 
	fn handle_assignment(&mut self, name: String, section: Section, mut pvar: PrepConfigVar) -> Result<(), String>
	{
		if let RawVar::StringVar(x) = pvar.var {
			pvar.var = RawVar::StringVar(self.interpolation(x, section, &name)?);
		}	
		println!("Making assignment {:?}: {} = {:?}", section, name, pvar.var);
		self.var.entry(name.clone()).or_insert_with(Vec::new);	
		// We know the value exists as this is assured by the previous line
		self.var.get_mut(&name).unwrap().push(pvar);
		Ok(())
	}
	fn check_var_and_env(&self, vname: &str, vname1: &str, section: Section) -> Option<String> {
		let stored_var = match self.get(vname, section) {
			Some(pv) => { if let RawVar::StringVar(st) = &pv.var { Some(st.clone()) } else { None }},
			None => { None },
		};
		// Apparently env::var_os() if the name contains an equals or Nul character
		if stored_var.is_none() && !(vname1.is_empty() || vname1.contains('=') || vname1.contains('\0')) {
			match env::var(vname1) {
				Ok(st) => { Some(st) },
				Err(_) => None,
			}
		} else {
			stored_var
		}	
	}
	fn interpolation(&self, var_str: String, section: Section, name: &str) -> Result<String, String> {
		let mut v = Vec::new();
		find_var(&var_str, &mut v);
		if v.is_empty() {return Ok(var_str); }
		if let Segment::End(_) = v[0] { return Ok(var_str); }
		// In the following section we can use unwrap() etc. because the indexes should be correct, and
		// if not then we should panic!
		let mut buf = String::new();
		for seg in v.iter() {
			match seg {
				Segment::NameOnly(idx) => {
					let vname = var_str.get(idx[2]..idx[3]).unwrap();
					let stored_var = self.check_var_and_env(vname, vname, section);
					buf.push_str(var_str.get(idx[0]..idx[1]).unwrap());
					match stored_var { 
						Some(x) => buf.push_str(&x),
						None => warn!("Unknown variable '{}' in assignment for variable '{}'", vname, name),
					}					
				},
				Segment::SectionName(idx) => {
					let vname = var_str.get(idx[4]..idx[5]).unwrap();
					let vname1 = var_str.get(idx[2]..idx[5]).unwrap();
					let sec = if idx[3] != idx[2] { 
						match Section::from_str(var_str.get(idx[2]..idx[3]).unwrap()) {
							Ok(s) => s,
							Err(_) => return Err(format!("Unknown Section '{}' in assignment for variable '{}'", vname, name)),
						}
					} else { section };
					let stored_var = self.check_var_and_env(vname, vname1, sec);					
					buf.push_str(var_str.get(idx[0]..idx[1]).unwrap());
					match stored_var { 
						Some(x) => buf.push_str(&x),
						None => warn!("Unknown variable '{}' in assignment for variable '{}'", vname, name),
					}					
				},
				Segment::End(idx) => {
					buf.push_str(var_str.get(*idx..).unwrap());
				},
			}
		}	
		Ok(buf)
	}	
	pub fn parse(&mut self) -> Result<(), String> {
		let mut state = ParserState::WaitingForName; 
		loop {
			let s = self.lexer.get_token()?;
			match state {
				ParserState::WaitingForName => {
					state = self.handle_name(s)?;
				},
				ParserState::WaitingForValue(name) => {
					state = self.handle_value(s, name)?;				
				},
				ParserState::AfterValue(x) => {
					state = self.handle_after_value(s, x)?;					
				},
				_ => (),
			}
			if let ParserState::End = state { break; } 
		}
		for(name, v) in self.var.iter_mut() {
			for pv in v {
				match pv.vtype {
					VarType::StringVar => { },
					VarType::FloatVec => { 
						if let RawVar::StringVec(vv) = &pv.var {
							let mut v = Vec::new();
							for s in vv.iter() {
								let rv = get_raw_var_from_string(s.to_string(), VarType::FloatVar, &name)?;
								if let RawVar::FloatVar(x) = rv { v.push(x); }
							}	
							pv.var = RawVar::FloatVec(v);					
						}	
					},
					_ => {
						if let RawVar::StringVar(var_str) = &pv.var {
							pv.var = get_raw_var_from_string(var_str.to_string(), pv.vtype, &name)?;
						}					
					},
				}
				println!("HASH: {:?}:{} = {:?}", pv.section, name, pv.var);
			}
		}
		Ok(())
	}	
}

fn get_raw_var_from_string(val_string: String, vtype: VarType, name: &str) -> Result<RawVar, String> {
	match vtype {
		VarType::StringVar => Ok(RawVar::StringVar(val_string)),
		VarType::IntVar => match val_string.parse::<isize>() {
			Ok(x) => Ok(RawVar::IntVar(x)),
			Err(_) => Err(format!("Error for variable {} when converting '{}' to integer value", name, val_string)),
		},
		VarType::BoolVar => match val_string.to_lowercase().parse::<bool>() {
			Ok(x) => Ok(RawVar::BoolVar(x)),
			Err(_) => Err(format!("Error for variable {} when converting '{}' to boolean value", name, val_string)),
		},
		VarType::FloatVar | VarType::FloatVec => match val_string.parse::<f64>() {
			Ok(x) => Ok(RawVar::FloatVar(x)),
			Err(_) => Err(format!("Error for variable {} when converting '{}' to float value", name, val_string)),
		},		
	}		
}
	



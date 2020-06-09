use std::collections::HashMap;
use std::str::FromStr;
use std::env;
use std::path::{PathBuf, Path};

use crate::common::defs::*;
use crate::config::GemBS;
	
mod lex;
mod find_var;
use lex::{Lexer, LexToken};
use find_var::{find_var, Segment};

#[derive(Debug, Clone)]
struct PrepConfigVar {
	var: DataValue,
	vtype:VarType,
	section: Section,
	known: bool,
	used: bool,
}

#[derive(Debug)]
struct KnownVar {
	vtype: VarType,
	sections: Vec<Section>
}

impl KnownVar {
	fn new(vtype: VarType, sections: Vec<Section>) -> Self {
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
		self.known_var.get(&tstr.as_str()).and_then(|v| if v.sections.contains(&section) { Some(v.vtype) } else { None })			
	}
}

fn make_known_var_list() -> KnownVarList {
	let mut kv_list = KnownVarList::new();
	kv_list.add_known_var("index", VarType::String, vec!(Section::Index));
	kv_list.add_known_var("nonbs_index", VarType::String, vec!(Section::Index));
	kv_list.add_known_var("index_dir", VarType::String, vec!(Section::Index));
	kv_list.add_known_var("reference", VarType::String, vec!(Section::Index));
	kv_list.add_known_var("extra_references", VarType::String, vec!(Section::Index));
	kv_list.add_known_var("reference_basename", VarType::String, vec!(Section::Index));
	kv_list.add_known_var("contig_sizes", VarType::String, vec!(Section::Index));
	kv_list.add_known_var("dbsnp_files", VarType::StringVec, vec!(Section::Index));
	kv_list.add_known_var("dbsnp_index", VarType::String, vec!(Section::Index));
	kv_list.add_known_var("sampling_rate", VarType::Int, vec!(Section::Index));
	kv_list.add_known_var("populate_cache", VarType::Bool, vec!(Section::Index));
	kv_list.add_known_var("threads", VarType::Int, vec!(Section::Index, Section::Mapping, Section::Calling, Section::Extract, Section::Report));
	kv_list.add_known_var("merge_threads", VarType::Int, vec!(Section::Mapping, Section::Calling));
	kv_list.add_known_var("map_threads", VarType::Int, vec!(Section::Mapping));
	kv_list.add_known_var("sort_threads", VarType::Int, vec!(Section::Mapping));
	kv_list.add_known_var("sort_memory", VarType::String, vec!(Section::Mapping));
	kv_list.add_known_var("non_stranded", VarType::Bool, vec!(Section::Mapping));
	kv_list.add_known_var("reverse_conversion", VarType::Bool, vec!(Section::Mapping));
	kv_list.add_known_var("remove_individual_bams", VarType::Bool, vec!(Section::Mapping));
	kv_list.add_known_var("underconversion_sequence", VarType::String, vec!(Section::Mapping));
	kv_list.add_known_var("overconversion_sequence", VarType::String, vec!(Section::Mapping));
	kv_list.add_known_var("bam_dir", VarType::String, vec!(Section::Mapping));
	kv_list.add_known_var("centre", VarType::String, vec!(Section::Mapping));
	kv_list.add_known_var("center", VarType::String, vec!(Section::Mapping));
	kv_list.add_known_var("platform", VarType::String, vec!(Section::Mapping));
	kv_list.add_known_var("sequence_dir", VarType::String, vec!(Section::Mapping));
	kv_list.add_known_var("benchmark_mode", VarType::Bool, vec!(Section::Mapping, Section::Calling));
	kv_list.add_known_var("make_cram", VarType::Bool, vec!(Section::Mapping));
	kv_list.add_known_var("jobs", VarType::Int, vec!(Section::Calling, Section::Extract));
	kv_list.add_known_var("bcf_dir", VarType::String, vec!(Section::Calling));
	kv_list.add_known_var("mapq_threshold", VarType::Int, vec!(Section::Calling));
	kv_list.add_known_var("qual_threshold", VarType::Int, vec!(Section::Calling));
	kv_list.add_known_var("left_trim", VarType::Int, vec!(Section::Calling));
	kv_list.add_known_var("right_trim", VarType::Int, vec!(Section::Calling));
	kv_list.add_known_var("species", VarType::String, vec!(Section::Calling));
	kv_list.add_known_var("keep_duplicates", VarType::Bool, vec!(Section::Calling));
	kv_list.add_known_var("keep_improper_pairs", VarType::Bool, vec!(Section::Calling));
	kv_list.add_known_var("call_threads", VarType::Int, vec!(Section::Calling));
	kv_list.add_known_var("remove_individual_bcfs", VarType::Bool, vec!(Section::Calling));
	kv_list.add_known_var("haploid", VarType::Bool, vec!(Section::Calling));
	kv_list.add_known_var("reference_bias", VarType::Float, vec!(Section::Calling, Section::Extract));
	kv_list.add_known_var("over_conversion_rate", VarType::Float, vec!(Section::Calling));
	kv_list.add_known_var("under_conversion_rate", VarType::Float, vec!(Section::Calling));
	kv_list.add_known_var("conversion", VarType::FloatVec, vec!(Section::Calling));
	kv_list.add_known_var("contig_list", VarType::StringVec, vec!(Section::Calling));
	kv_list.add_known_var("contig_pool_limit", VarType::Int, vec!(Section::Calling));
	kv_list.add_known_var("extract_dir", VarType::String, vec!(Section::Extract));
	kv_list.add_known_var("snp_list", VarType::String, vec!(Section::Extract));
	kv_list.add_known_var("snp_db", VarType::String, vec!(Section::Extract));
	kv_list.add_known_var("allow_het", VarType::Bool, vec!(Section::Extract));
	kv_list.add_known_var("phred_threshold", VarType::Int, vec!(Section::Extract));
	kv_list.add_known_var("min_inform", VarType::Int, vec!(Section::Extract));
	kv_list.add_known_var("extract_threads", VarType::Int, vec!(Section::Extract));
	kv_list.add_known_var("min_nc", VarType::Int, vec!(Section::Extract));
	kv_list.add_known_var("make_cpg", VarType::Bool, vec!(Section::Extract));
	kv_list.add_known_var("make_non_cpg", VarType::Bool, vec!(Section::Extract));
	kv_list.add_known_var("make_bedmethyl", VarType::Bool, vec!(Section::Extract));
	kv_list.add_known_var("make_snps", VarType::Bool, vec!(Section::Extract));
	kv_list.add_known_var("bigwig_strand_specific", VarType::Bool, vec!(Section::Extract));
	kv_list.add_known_var("strand_specific", VarType::Bool, vec!(Section::Extract));
	kv_list.add_known_var("project", VarType::String, vec!(Section::Report));
	kv_list.add_known_var("report_dir", VarType::String, vec!(Section::Report));
	kv_list
}

#[derive(Debug)]
enum ParserState { 
	WaitingForName, 
	WaitingForValue(String), 
	AfterValue((String, Section, PrepConfigVar)), 
	End 
}	

struct PrepConfig {
	kv_list: KnownVarList,
	var: HashMap<String, Vec<PrepConfigVar>>,
	lexer: Lexer,
}

impl PrepConfig {
	fn new(config_script_path: &Path) -> Self {
		PrepConfig{
			kv_list: make_known_var_list(), 
			var: HashMap::new(), 
			lexer: Lexer::new(config_script_path)
		}
	}
	
	fn check_vtype(&self, name: &str, section: Section) -> Option<VarType> {
		self.kv_list.check_vtype(name, section)		
	}
	
	fn start_parse(&mut self, name: &str) -> Result<(), String> {
		self.lexer.init_lexer(name)
	}
	
	fn get(&mut self, name: &str, section: Section) -> Option<&mut PrepConfigVar> {
		if name.is_empty() { return None; }
		self.var.get_mut(&name.to_lowercase()).and_then(|v| {
			let mut default_var = None;
			let mut var = None;
			for pv in v.iter_mut() {
				if pv.section == section { var = Some(pv) } else if pv.section == Section::Default { default_var = Some(pv) }
			}
			var.or(default_var)
		})
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
				let vt = vt.unwrap_or(VarType::String);

				// We initially keep everything as a string until after finishing parsing the config file(s)
				// To allow interpolation to work as expected
				let rv = match vt {
					VarType::FloatVec | VarType::StringVec => DataValue::StringVec(vec!(val_str;1)),
					_ => DataValue::String(val_str),
				};

				let pvar = PrepConfigVar{var: rv, vtype: vt, section, known, used: false};
				Ok(ParserState::AfterValue((name, section, pvar)))
			},
			_ => Err(format!("Unexpected token - waiting for value after variable {}", name)),
		}
	}
	
	fn handle_next_value(&mut self, val_str: String, mut pvar: PrepConfigVar, name: String, section: Section) -> Result<ParserState, String> {
		let mut vector = match pvar.var {
			DataValue::StringVec(x) => x,
			_ => return Err("Internal error in handle_next_value()".to_string()),
		};
		vector.push(val_str);
		pvar.var = DataValue::StringVec(vector);
		Ok(ParserState::AfterValue((name, section, pvar)))
	}
		
	fn handle_after_value(&mut self, tok: LexToken, x: (String, Section, PrepConfigVar)) -> Result<ParserState, String> {
		let name = x.0;
		let section = x.1;
		let pvar = x.2;
		let vector = if let DataValue::StringVec(_) = pvar.var { true } else { false };
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
	
	fn handle_assignment(&mut self, name: String, section: Section, mut pvar: PrepConfigVar) -> Result<(), String> {
		if let DataValue::String(x) = pvar.var {
			pvar.var = DataValue::String(self.interpolation(x, section, &name)?);
		}	
		trace!("Making assignment {:?}: {} = {:?}", section, name, pvar.var);
		self.var.entry(name).or_insert_with(Vec::new).push(pvar);	
		Ok(())
	}
	
	fn check_var_and_env(&mut self, vname: &str, vname1: &str, section: Section) -> Option<String> {
		let stored_var = self.get(vname, section).and_then(|pv| {
			if let DataValue::String(st) = &pv.var {
				pv.used = true; 
				Some(st.clone()) 
			} else { None }	
		});	
		// Apparently env::var_os() can panic if the name contains an equals or Nul character
		stored_var.or_else(|| {
			if !(vname1.is_empty() || vname1.contains('=') || vname1.contains('\0')) {
				match env::var(vname1) {
					Ok(st) => { Some(st) },
					Err(_) => None,
				}
			} else { None }
		})
	}
	
	fn interpolation(&mut self, var_str: String, section: Section, name: &str) -> Result<String, String> {
		let mut v = Vec::new();
		find_var(&var_str, &mut v);
		if v.is_empty() { return Ok(var_str); }
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
		
	fn parse(&mut self, gem_bs: &mut GemBS) -> Result<(), String> {
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
				if pv.known {
					let rv = match pv.vtype {
						VarType::String => { Some(pv.var.clone()) },
						VarType::FloatVec => { 
							if let DataValue::StringVec(vv) = &pv.var {
								let mut v = Vec::new();
								for s in vv.iter() {
									if let DataValue::Float(val) = DataValue::from_str(s, VarType::Float)? { v.push(val); }
								}	
								Some(DataValue::FloatVec(v))					
							} else { None }
						},
						VarType::StringVec => { 
							if let DataValue::StringVec(vv) = &pv.var {
								let mut v = Vec::new();
								for s in vv.iter() { v.push(s.clone()); }
								Some(DataValue::StringVec(v))					
							} else { None }
						},
						_ => {
							if let DataValue::String(var_str) = &pv.var {
								Some(DataValue::from_str(&var_str, pv.vtype)?)
							} else { None }				
						},
					};
					if let Some(v) = rv { gem_bs.set_config(pv.section, &name, v); }
				} else if !pv.used {
					warn!("Warning: Variable '{}' in section '{:?}' not used", name, pv.section);
				}
			}
		}
		Ok(())
	}	
}

pub fn process_config_file(file_name: &str, gem_bs: &mut GemBS) -> Result<(), String> {
	let mut prep_config = PrepConfig::new(&gem_bs.get_config_script_path());
	prep_config.start_parse(file_name)?;
	prep_config.parse(gem_bs)?;
	Ok(())
}

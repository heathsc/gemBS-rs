use std::str::FromStr;
use std::io;
use crate::config::*;

use clap::ArgMatches;

pub fn get_option(m: &ArgMatches, opt: &str, default: ConfVar) -> io::Result<ConfVar> {
	match default {
		ConfVar::Bool(_) => if m.is_present(opt) { Ok(ConfVar::Bool(true)) } else { Ok(default) },
		ConfVar::String(_) => m.value_of(opt).map(|x| Ok(ConfVar::String(Some(x.to_owned())))).unwrap_or(Ok(default)),
		ConfVar::Float(_) => {
			if let Some(x) = m.value_of(opt) { 
				<f64>::from_str(x).map(ConfVar::Float).map_err(|e| new_err(format!("Couldn't parse float argument '{}' for option {}: {}", x, opt, e)))
			} else { Ok(default) }	
		}
		ConfVar::Int(_) => {
			if let Some(x) = m.value_of(opt) { 
				<usize>::from_str(x).map(ConfVar::Int).map_err(|e| new_err(format!("Couldn't parse integer argument '{}' for option {}: {}", x, opt, e)))
			} else { Ok(default) }	
		}
		ConfVar::Mode(_) => {
			let s = m.value_of(opt).map(|x| x.to_ascii_lowercase());
			match s.as_deref() {
				Some("combined") => Ok(ConfVar::Mode(Mode::Combined)),
				Some("strand-specific") => Ok(ConfVar::Mode(Mode::StrandSpecific)),
				Some(s) => Err(new_err(format!("Couldn't parse argument '{}' for option {}", s, opt))),
				None => Ok(default),
			}	 
		},	
		ConfVar::Select(_) => {
			let s = m.value_of(opt).map(|x| x.to_ascii_lowercase());
			match s.as_deref() {
				Some("hom") => Ok(ConfVar::Select(Select::Hom)),
				Some("het") => Ok(ConfVar::Select(Select::Het)),
				Some(s) => Err(new_err(format!("Couldn't parse argument '{}' for option {}", s, opt))),
				None => Ok(default),
			}	 
		},	
	}
}

pub fn get_fvec(m: &ArgMatches, opt: &str, low: f64, high: f64) -> io::Result<Option<Vec<f64>>> {
	if let Some(v) = m.values_of(opt) {
		let mut vec = Vec::new();
		for x in v {
			match <f64>::from_str(x) {
				Ok(z) => {
					if z >= low && z <= high { vec.push(z) }
					else { return Err(new_err(format!("Float argument '{}' for option {} not between {} and {}", x, opt, low, high))) }
				},
				Err(e) => return Err(new_err(format!("Couldn't parse float argument '{}' for option {}: {}", x, opt, e))),
			}
		}
		Ok(Some(vec))
	} else { Ok(None) }
}

pub fn get_f64(m: &ArgMatches, opt: &str, low: f64, high: f64) -> io::Result<Option<f64>> {
	if let Some(x) = m.value_of(opt) {
		Ok(match <f64>::from_str(x) {
			Ok(z) => {
				if z >= low && z <= high { Some(z) }
				else { return Err(new_err(format!("Float argument '{}' for option {} not between {} and {}", x, opt, low, high))) }
			},
			Err(e) => return Err(new_err(format!("Couldn't parse float argument '{}' for option {}: {}", x, opt, e))),
		})
	} else { Ok(None) }
}

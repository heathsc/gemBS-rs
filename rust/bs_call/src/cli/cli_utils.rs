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
		ConfVar::OType(_) => {
			if let Some(x) = m.value_of(opt) { 
				<OType>::from_str(x).map(ConfVar::OType).map_err(|e| new_err(format!("Couldn't parse output type argument '{}' for option {}: {}", x, opt, e)))
			} else { Ok(default) }	
		}		
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

pub fn get_ivec(m: &ArgMatches, opt: &str, low: usize, high: usize) -> io::Result<Option<Vec<usize>>> {
	if let Some(v) = m.values_of(opt) {
		let mut vec = Vec::new();
		for x in v {
			match <usize>::from_str(x) {
				Ok(z) => {
					if z >=low && z <= high{ vec.push(z) }
					else { return Err(new_err(format!("Integer argument '{}' for option {} not between {} and {}", x, opt, low, high))) }
				},
				Err(e) => return Err(new_err(format!("Couldn't parse integer argument '{}' for option {}: {}", x, opt, e))),
			}
		}
		Ok(Some(vec))
	} else { Ok(None) }
}

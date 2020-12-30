use std::str::FromStr;
use std::io;

use clap::ArgMatches;
use crate::config::*;

pub fn get_arg_string<S: AsRef<str>>(m: &ArgMatches, s: S) -> Option<String> { m.value_of(s.as_ref()).map(|x| x.to_owned()) }
pub fn get_arg_f64<S: AsRef<str>>(m: &ArgMatches, s: S) -> io::Result<Option<f64>> { 
	let s = s.as_ref();
	if let Some(x) = m.value_of(s) {
		match <f64>::from_str(x) {
			Ok(z) => Ok(Some(z)),
			Err(e) => Err(new_err(format!("Couldn't parse float argument '{}' for option {}: {}", x, s, e))),
		}
	} else { Ok(None) }
}
pub fn get_arg_usize<S: AsRef<str>>(m: &ArgMatches, s: S) -> io::Result<Option<usize>> { 
	let s = s.as_ref();
	if let Some(x) = m.value_of(s) {
		match <usize>::from_str(x) {
			Ok(z) => Ok(Some(z)),
			Err(e) => Err(new_err(format!("Couldn't parse integer argument '{}' for option {}: {}", x, s, e))),
		}
	} else { Ok(None) }
}
pub fn get_arg_itype<S: AsRef<str>>(m: &ArgMatches, s: S) -> io::Result<IType> { 
	if let Some(x) = m.value_of(s.as_ref()) {
		match x.to_lowercase().as_str() {
			"auto" => Ok(IType::Auto),
			"bed" => Ok(IType::Bed),
			"vcf" => Ok(IType::Vcf),
			"json" => Ok(IType::Json),
			_ => Err(new_err(format!("Unrecognized input type: {}", x))),
		}
	} else { Ok(IType::Auto) }
}


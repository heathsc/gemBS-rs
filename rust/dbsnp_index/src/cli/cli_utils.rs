use std::io;

use clap::ArgMatches;
use crate::config::*;

pub fn get_arg_string<S: AsRef<str>>(m: &ArgMatches, s: S) -> Option<String> {
	m.get_one::<String>(s.as_ref()).map(|x| x.to_owned())
}

pub fn get_arg_f64<S: AsRef<str>>(m: &ArgMatches, s: S) -> io::Result<Option<f64>> {
	let s = s.as_ref();
	match m.try_get_one::<f64>(s) {
		Ok(z) => Ok(z.copied()),
		Err(e) => Err(new_err(format!("Couldn't parse float argument for option {}: {}", s, e))),
	}
}

pub fn get_arg_usize<S: AsRef<str>>(m: &ArgMatches, s: S) -> io::Result<Option<usize>> { 
	let s = s.as_ref();
	match m.try_get_one::<usize>(s) {
		Ok(z) => Ok(z.copied()),
		Err(e) => Err(new_err(format!("Couldn't parse integer argument for option {}: {}", s, e))),
	}
}

pub fn get_arg_itype<S: AsRef<str>>(m: &ArgMatches, s: S) -> io::Result<IType> {
	let s = s.as_ref();
	match m.try_get_one::<String>(s) {
		Ok(x) => match x.expect("Missing type").to_lowercase().as_str() {
			"auto" => Ok(IType::Auto),
			"bed" => Ok(IType::Bed),
			"vcf" => Ok(IType::Vcf),
			"json" => Ok(IType::Json),
			_ => Err(new_err("Unrecognized input type".to_string())),
		}
		Err(e) => Err(new_err(format!("Couldn't parse Input Type argument for option {}: {}", s, e))),
	}
}


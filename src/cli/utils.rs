use clap::ArgMatches;
use std::str::FromStr;

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

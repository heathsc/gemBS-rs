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

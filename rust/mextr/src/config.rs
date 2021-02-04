use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};

pub fn new_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

#[derive(Debug,Copy, Clone)]
pub enum Mode { Combined, StrandSpecific }

#[derive(Debug,Copy, Clone)]
pub enum Select { Hom, Het }

#[derive(Debug,Clone)]
pub enum ConfVar {
	Bool(bool),
	Int(usize),
	Float(f64),
	String(Option<String>),
	Mode(Mode),
	Select(Select),
}

#[derive(Clone)]
pub struct ConfHash {
	hash: HashMap<&'static str, ConfVar>,
}

impl ConfHash {
	pub fn new(hash: HashMap<&'static str, ConfVar>) -> Self { ConfHash {hash} }
	pub fn get(&self,  key: &str) -> Option<&ConfVar> { self.hash.get(key) }
	pub fn set(&mut self, key: &'static str, val: ConfVar) { self.hash.insert(key, val); }

	pub fn get_bool(&self, key: &str) -> bool { 
		if let Some(ConfVar::Bool(x)) = self.get(key) { *x } else { panic!("Bool config var {} not set", key); }
	}
	
	pub fn get_int(&self, key: &str) -> usize { 
		if let Some(ConfVar::Int(x)) = self.get(key) { *x } else { panic!("Integer config var {} not set", key); }
	}

	pub fn get_float(&self, key: &str) -> f64 { 
		if let Some(ConfVar::Float(x)) = self.get(key) { *x } else { panic!("Flaot config var {} not set", key); }
	}

	pub fn get_str(&self, key: &str) -> Option<&str> { 
		if let Some(ConfVar::String(x)) = self.get(key) { x.as_deref() } else { panic!("String config var {} not set", key); }
	}
	pub fn get_select(&self, key: &str) -> Select { 
		if let Some(ConfVar::Select(x)) = self.get(key) { *x } else { panic!("Select config var {} not set", key); }
	}
	pub fn get_mode(&self, key: &str) -> Mode { 
		if let Some(ConfVar::Mode(x)) = self.get(key) { *x } else { panic!("Bool config var {} not set", key); }
	}
}


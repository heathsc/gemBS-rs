use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct PrefixHash {
	prefix_hash: RwLock<HashMap<Arc<str>, u32>>,
}

impl PrefixHash {
	pub fn new() -> Self { Self{ prefix_hash: RwLock::new(HashMap::new()) }}
	
	pub fn mk_lookup(&self) -> PrefixLookup {
		PrefixLookup{cache: None, prefix_hash: self}
	}	
	
	pub fn get_prefix(&self, name: &str) -> (Arc<str>, u32) {
		let hash = self.prefix_hash.read().unwrap();
		if let Some((k, x)) = hash.get_key_value(name) {
			trace!("Got prefix {} ({}) from hash", name, x); 
			return (k.clone(), *x) 
		}
		let x = hash.len() as u32;
		assert!(x < 0xffffffff);
		drop(hash);
		debug!("Adding new prefix {} ({})", name, x);
		let mut hash = self.prefix_hash.write().unwrap();
		let k: Arc<str> = Arc::from(name);
		hash.insert(k.clone(), x);
		(k, x)
	}
}

impl Default for PrefixHash {
	fn default() -> Self { PrefixHash::new() }
}

struct PrefixCache {
	name: Arc<str>,
	prefix: u32,	
}

pub struct PrefixLookup<'a> {
	cache: Option<PrefixCache>,
	prefix_hash: &'a PrefixHash,
}

impl <'a> PrefixLookup<'a> {
	pub fn get_prefix(&mut self, name: &str) -> u32 {
		if let Some(c) = &self.cache {
			if c.name.as_ref() == name { 
				trace!("Got prefix {} ({}) from cache", name, c.prefix);
				return c.prefix;
			}
		}
		trace!("Looking up prefix {}", name);
		let (k, x) = self.prefix_hash.get_prefix(name);
		self.cache = Some(PrefixCache{name: k, prefix: x});
		x
	}
}

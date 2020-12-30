use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub struct Contig {
	name: Arc<str>,
}

impl Contig {
	pub fn name(&self) -> &str { &self.name }
}

pub struct ContigHash {
	contig_hash: RwLock<HashMap<Arc<str>, Arc<Contig>>>,
}

impl ContigHash {
	pub fn new() -> Self { Self{ contig_hash: RwLock::new(HashMap::new()) }}
	
	pub fn mk_lookup(&self) -> ContigLookup {
		ContigLookup{cache: None, contig_hash: self}
	}	
	
	pub fn get_contig(&self, name: &str) -> Arc<Contig> {
		let hash = self.contig_hash.read().unwrap();
		if let Some(ctg) = hash.get(name) {
			trace!("Got contig {} from hash", name); 
			return ctg.clone() 
		}
		drop(hash);
		debug!("Adding new contig {}", name);
		let mut hash = self.contig_hash.write().unwrap();
		let ctg = Arc::new(Contig{name: Arc::from(name)});
		hash.insert(Arc::from(name), ctg.clone());
		ctg
	}
}

impl Default for ContigHash {
	fn default() -> Self { ContigHash::new() }
}

struct ContigCache {
	name: Arc<str>,
	contig: Arc<Contig>,	
}

pub struct ContigLookup<'a> {
	cache: Option<ContigCache>,
	contig_hash: &'a ContigHash,
}

impl <'a> ContigLookup<'a> {
	pub fn get_contig(&mut self, name: &str) -> Arc<Contig> {
		if let Some(c) = &self.cache {
			if c.name.as_ref() == name { 
				trace!("Got contig {} from cache", name);
				return c.contig.clone() 
			}
		}
		trace!("Looking up contig {}", name);
		let ctg = self.contig_hash.get_contig(name);
		self.cache = Some(ContigCache{name: ctg.name.clone(), contig: ctg.clone()});
		ctg
	}
}


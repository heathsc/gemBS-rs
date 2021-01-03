use std::collections::HashMap;
use std::sync::{Arc, RwLock, TryLockError, RwLockWriteGuard};

use crossbeam_channel::{bounded, Sender, Receiver};
use super::snp::SnpBlock;

pub struct ContigData {
	recv: Receiver<SnpBlock>
	// all the contig data 
}

impl ContigData {
	pub fn recv(&self) -> &Receiver<SnpBlock> { &self.recv }	
}

pub struct Contig {
	name: Arc<str>,
	data: RwLock<ContigData>,
	send: Sender<SnpBlock>,
}

impl Contig {
	pub fn new<S: AsRef<str>>(name: S, channel_size: usize) -> Self {
		let (send, recv) = bounded(channel_size);
		let data = RwLock::new(ContigData{recv});
		Self {name: Arc::from(name.as_ref()), data, send}
	}
	pub fn send_message(&self, sb: SnpBlock) { self.send.send(sb).unwrap(); }
	pub fn try_get_recv(&self) -> Option<Receiver<SnpBlock>> {
		match self.data.try_read() {
			Ok(cdata) => Some(cdata.recv.clone()),
			Err(TryLockError::WouldBlock) => None,
			Err(_) => panic!("Error obtaining lock"),
		}
	}
	pub fn try_bind(&self) -> Option<RwLockWriteGuard<ContigData>> {
		match self.data.try_write() {
			Ok(guard) => Some(guard),
			Err(TryLockError::WouldBlock) => None,
			Err(_) => panic!("Error obtaining lock"),
		}
	}
	pub fn name(&self) -> &str { &self.name }
	pub fn ref_name(&self) -> Arc<str> { self.name.clone() }
}

pub struct ContigHash {
	contig_hash: RwLock<HashMap<Arc<str>, Arc<Contig>>>,
	channel_size: usize,
}

impl ContigHash {
	pub fn new(channel_size: usize) -> Self { Self{ contig_hash: RwLock::new(HashMap::new()), channel_size }}
	
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
		let mut hash = self.contig_hash.write().unwrap();
		if let Some(ctg) = hash.get(name) {
			trace!("Got contig {} from hash", name); 
			ctg.clone() 			
		} else {
			debug!("Adding new contig {}", name);
			let ctg = Arc::new(Contig::new(name, self.channel_size));
			hash.insert(Arc::from(name), ctg.clone());
			ctg
		}
	}
	pub fn get_avail_contig_list(&self) -> Vec<(Arc<Contig>, Receiver<SnpBlock>)> {
		let h = self.contig_hash.read().unwrap();
		let mut v = Vec::with_capacity(h.len());
		for ctg in h.values() {
			// See if receiver is available
			if let Some(r) = ctg.try_get_recv() { 
				v.push((ctg.clone(), r.clone())) 
			}
		}
		v
	}
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


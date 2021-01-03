use std::collections::HashMap;
use std::sync::{Arc, RwLock, TryLockError, RwLockWriteGuard};

use crossbeam_channel::{bounded, Sender, Receiver};
use super::snp::{RawSnp, SnpBlock};
use crate::config::Config;

#[derive(Clone, Default)]
pub struct ContigBin {
	mask: u64,
	fq_mask: u64,
	entries: Vec<u16>,
	name_buf: Vec<u8>,	
}


struct ContigInnerData {
	min_bin: usize,	
	bins: Vec<ContigBin>,
}

impl ContigInnerData {
	fn new(min_bin: usize, max_bin: usize) -> Self {
		let bins: Vec<ContigBin> = vec!(Default::default(); max_bin - min_bin + 1);
		Self{min_bin, bins}
	}
	fn check_min(&mut self, min_bin: usize) {
		if min_bin < self.min_bin {
			let mut v: Vec<ContigBin> = vec!(Default::default(); self.min_bin - min_bin);
			v.append(&mut self.bins);
			self.bins = v;
			self.min_bin = min_bin;
		}
	}
	fn check_max(&mut self, max_bin: usize) {
		let mb = self.min_bin + self.bins.len() - 1;
		if max_bin > mb {
			let mut v: Vec<ContigBin> = vec!(Default::default(); max_bin - mb);
			self.bins.append(&mut v);
		}
	}
}
pub struct ContigData {
	recv: Receiver<SnpBlock>,
	inner: Option<ContigInnerData>,
}

static DTAB: [u8; 256] = [
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 
	0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 	
];

impl ContigData {
	pub fn recv(&self) -> &Receiver<SnpBlock> { &self.recv }
	pub fn check_bins(&mut self, min: u32, max: u32) {
		assert!(max >= min);
		let min_bin = (min >> 6) as usize;
		let max_bin = (max >> 6) as usize;		
		if let Some(idata) = self.inner.as_mut() {
			idata.check_min(min_bin);
			idata.check_max(max_bin);
		} else { self.inner = Some(ContigInnerData::new(min_bin, max_bin)) }
	}
	pub fn add_snp(&mut self, snp: &RawSnp, conf: &Config) -> usize {
		let idata = self.inner.as_mut().unwrap();
		let bin = &mut idata.bins[((snp.pos() >> 6) as usize) - idata.min_bin];		
		let off = (snp.pos() & 63) as u16;
		let mask = 1 << off;
		if (bin.mask & mask) == 0 { 
			bin.mask |= mask;
			let name = snp.name();
			if name.len() > 510 { panic!("The name for SNP {} is too long (Max <= 510)", name)}
			let l1: u16 = ((name.len() + 1) >> 1) as u16;
			let select = {
				let s = if let Some(maf_limit) = conf.maf_limit() {
					if let Some(maf) = snp.maf() { maf >= maf_limit as f32 }
					else { true }
				} else { true };
				if !s { conf.selected(name) } else { s }
			};
			let n_entries = bin.entries.len() >> 1;
			if select { bin.fq_mask |= 1 << n_entries }
			bin.entries.extend_from_slice(&[(l1 << 8) | off, snp.prefix()]);
			let it = name.as_bytes().chunks_exact(2);
			let rem = it.remainder();
			for v in it { bin.name_buf.push((DTAB[v[0] as usize] << 4) | DTAB[v[1] as usize]) }
			for c in rem.iter() { bin.name_buf.push((DTAB[*c as usize] << 4) | 0xf) }		
			1
		} else { 0 }
	}
}

pub struct Contig {
	name: Arc<str>,
	data: RwLock<ContigData>,
	send: Sender<SnpBlock>,
}

impl Contig {
	pub fn new<S: AsRef<str>>(name: S, channel_size: usize) -> Self {
		let (send, recv) = bounded(channel_size);
		let data = RwLock::new(ContigData{recv, inner: None});
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


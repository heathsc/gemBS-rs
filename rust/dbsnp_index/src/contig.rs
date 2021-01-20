use std::collections::HashMap;
use std::sync::{Arc, RwLock, TryLockError, RwLockWriteGuard};
use std::io::Write;

use crossbeam_channel::{bounded, Sender, Receiver};
use super::snp::{RawSnp, SnpBlock};
use crate::config::Config;

#[derive(Clone, Default, Debug)]
pub struct ContigBin {
	mask: [u128; 2],
	half_full: bool,
	entries: Vec<u8>,
	name_buf: Vec<u8>,	
}

impl ContigBin {
	pub fn name_buf(&self) -> &[u8] { &self.name_buf }
	pub fn mask(&self) -> &[u128; 2] { &self.mask }
	pub fn entries(&self) -> &[u8] { &self.entries }
	fn sort_idx(&mut self) -> Vec<(u8, usize)> {
		let mut idx = Vec::with_capacity(256);
		let mut start_ix = 0;
		let mut left = 0; 
		let mut it = self.name_buf.iter();
		for off in self.entries.iter() {
			idx.push((*off, start_ix));
			start_ix += left;
			left = 0;
			loop {
				match it.next() {
					None => break,
					Some(x) if (x & 0xf0) >= 0xe0 => {
						start_ix += 1;
						left = 1;
						break;
					},
					Some(x) => {
						start_ix += 2;
						if (x & 0xf) >= 0xe { break }
					},
				}
			}
		}
		idx.sort_unstable_by_key(|(off, _)| *off);
		idx
	}
	
	// Write names sorted by position within bin
	pub fn write_names<W: Write>(&mut self, w: W) {
		let idx = self.sort_idx();
		let mut writer = NameWriter::new(w);
		for (_, x) in idx.iter() {
			let mut it = self.name_buf[x>>1..].iter(); 
			if (x & 1) == 1 { writer.write_u4(*it.next().expect("Short name buffer")) }
			loop {
				match it.next() {
					None => break,
					Some(x) if (x & 0xf0) >= 0xe0 => {
						writer.write_u4(x >> 4);
						break;
					},
					Some(x) => {
						writer.write_u8(*x);
						if (x & 0xf) >= 0xe { break }
					},
				}
			}
		}
	}
}

struct NameWriter<W: Write> {
	writer: W,
	buf: Option<u8>,	
}

impl <W: Write>NameWriter<W> {
	fn new(w: W) -> Self { Self{writer: w, buf: None} }
	
	fn write_u4(&mut self, c: u8) {
		if let Some(x) = self.buf.take() { self.writer.write_all(&[x | (c & 0xf)]).expect("Write error") }
		else { self.buf = Some(c << 4) }
	}
	
	fn write_u8(&mut self, c: u8) {
		if let Some(x) = self.buf {
			self.writer.write_all(&[x | (c >> 4)]).expect("Write error");
			self.buf = Some(c << 4);
		} else { self.writer.write_all(&[c]).expect("Write error") }
	}
}

impl <W: Write> Drop for NameWriter<W> {
	fn drop(&mut self) {
		if let Some(c) = self.buf.take() { self.writer.write_all(&[c]).expect("Write error") }
	}	
}

#[derive(Debug)]
struct ContigInnerData {
	min_bin: usize,	
	max_bin: usize,
	bins: Vec<Option<ContigBin>>,
}

#[derive(Default, Debug, Copy, Clone)]
pub struct ContigStats {
	n_snps: usize,
	n_selected_snps: usize,
	n_non_empty_bins: usize,	
}

impl ContigStats {
	pub fn n_snps(&self) -> usize { self.n_snps }	
	pub fn n_selected_snps(&self) -> usize { self.n_selected_snps }	
	pub fn n_non_empty_bins(&self) -> usize { self.n_non_empty_bins }	
}

impl ContigInnerData {
	fn new(min_bin: usize, max_bin: usize) -> Self {
		let bins: Vec<Option<ContigBin>> = vec!(None; max_bin - min_bin + 1);
		Self{min_bin, max_bin, bins }
	}
	fn check_min(&mut self, min_bin: usize) {
		if min_bin < self.min_bin {
			let mut v: Vec<Option<ContigBin>> = vec!(None; self.min_bin - min_bin);
			v.append(&mut self.bins);
			self.bins = v;
			self.min_bin = min_bin;
		}
	}
	fn check_max(&mut self, max_bin: usize) {
		let mb = self.min_bin + self.bins.len() - 1;
		if max_bin > mb {
			let mut v: Vec<Option<ContigBin>> = vec!(None; max_bin - mb);
			self.bins.append(&mut v);
			self.max_bin = max_bin;
		}
	}
}

#[derive(Debug)]
pub struct ContigData {
	recv: Receiver<SnpBlock>,
	inner: Option<ContigInnerData>,
	stats: ContigStats,
}

impl ContigData {
	pub fn recv(&self) -> &Receiver<SnpBlock> { &self.recv }
	pub fn check_bins(&mut self, min: u32, max: u32) {
		assert!(max >= min);
		let min_bin = (min >> 8) as usize;
		let max_bin = (max >> 8) as usize;		
		if let Some(idata) = self.inner.as_mut() {
			idata.check_min(min_bin);
			idata.check_max(max_bin);
		} else { self.inner = Some(ContigInnerData::new(min_bin, max_bin)) }
	}
	pub fn add_snp(&mut self, snp: &RawSnp, conf: &Config) {
		let idata = self.inner.as_mut().unwrap();
		let bin_idx = ((snp.pos() >> 8) as usize) - idata.min_bin;
		if idata.bins[bin_idx].is_none() { 
			self.stats.n_non_empty_bins += 1;
			idata.bins[bin_idx] = Some(Default::default());
		};
		let bin = idata.bins[bin_idx].as_mut().unwrap();	
		let off = (snp.pos() & 255) as u8;
		let (ix, off1) = if off < 128 { (0, off) } else { (1, off & 127) };
		let mask = 1u128 << off1;
		if (bin.mask[ix] & mask) == 0 { 
			self.stats.n_snps += 1;
			bin.mask[ix] |= mask;
			let name = snp.name();
			if name.len() > 254 { panic!("The name for SNP {} is too long (Max <= 254)", name)}
			let select = {
				let s = if let Some(maf_limit) = conf.maf_limit() {
					if let Some(maf) = snp.maf() { maf >= maf_limit as f32 }
					else { false }
				} else { false };
				if !s { conf.selected(name) } else { false }
			};
			let term_code = if select { 
				self.stats.n_selected_snps += 1;
				0xf 
			} else { 0xe };
			let nbuf = name.as_bytes();
			let it = if bin.half_full {
				*bin.name_buf.last_mut().unwrap() |= nbuf.first().unwrap() - b'0';
				&nbuf[1..]
			} else { nbuf }.chunks_exact(2);
			let rem = it.remainder();
			for v in it { bin.name_buf.push(((v[0] - b'0') << 4) | (v[1] - b'0'))}
			if rem.is_empty() {
				bin.name_buf.push(term_code << 4);
				bin.half_full = true;
			} else {
				bin.name_buf.push(((rem.first().unwrap() - b'0') << 4) | term_code);
				bin.half_full = false;
			}	
			bin.entries.push(off);
		}
	}
	pub fn stats(&self) -> &ContigStats { &self.stats }
	pub fn min_bin(&self) -> Option<usize> { self.inner.as_ref().map(|x| x.min_bin) }
	pub fn max_bin(&self) -> Option<usize> { self.inner.as_ref().map(|x| x.max_bin) }
	pub fn min_max(&self) -> Option<(usize, usize)> { self.inner.as_ref().map(|x| (x.min_bin, x.max_bin)) }
	pub fn bins(&mut self) -> Option<&mut Vec<Option<ContigBin>>> {
		if let Some(idata) = &mut self.inner { Some(&mut idata.bins) }
		else { None }
	}
} 

#[derive(Debug)]
pub struct Contig {
	name: Arc<str>,
	data: RwLock<ContigData>,
	send: Sender<SnpBlock>,
}

impl Contig {
	pub fn new<S: AsRef<str>>(name: S, channel_size: usize) -> Self {
		let (send, recv) = bounded(channel_size);
		let data = RwLock::new(ContigData{recv, inner: None, stats: Default::default()});
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
	pub fn stats(&self) -> Option<ContigStats> {
		match self.data.try_read() {
			Ok(cdata) => Some(*cdata.stats()),
			Err(_) => None,
		}
	}
	pub fn data(&self) -> &RwLock<ContigData> { &self.data }
}

pub struct ContigHash {
	contig_hash: RwLock<HashMap<Arc<str>, Arc<Contig>>>,
	chrom_alias: Option<HashMap<String, String>>,
	channel_size: usize,
}

impl ContigHash {
	pub fn new(channel_size: usize, chrom_alias: Option<HashMap<String, String>>) -> Self { Self{ contig_hash: RwLock::new(HashMap::new()), chrom_alias, channel_size }}
	
	pub fn mk_lookup(&self) -> ContigLookup {
		ContigLookup{cache: None, contig_hash: self}
	}	
	
	pub fn get_contig(&self, name: &str) -> Option<Arc<Contig>> {
		// Check for contig alias
		let name = if let Some(h) = &self.chrom_alias { 
			trace!("Checking for alias for {}", name);
			h.get(name)? 
		} else { name };
		trace!("Looking up {} from hash", name);
		let hash = self.contig_hash.read().unwrap();
		if let Some(ctg) = hash.get(name) {
			trace!("Got contig {} from hash", name); 
			return Some(ctg.clone()) 
		}
		drop(hash);
		let mut hash = self.contig_hash.write().unwrap();
		if let Some(ctg) = hash.get(name) {
			trace!("Got contig {} from hash", name); 
			Some(ctg.clone()) 			
		} else {
			debug!("Adding new contig {}", name);
			let ctg = Arc::new(Contig::new(name, self.channel_size));
			hash.insert(Arc::from(name), ctg.clone());
			Some(ctg)
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
	pub fn get_ctg_stats(&self) -> Vec<(Arc<Contig>, ContigStats)> {
		let h = self.contig_hash.read().unwrap();
		let mut v = Vec::with_capacity(h.len());
		for ctg in h.values() {
			let stats = ctg.stats().expect("Couldn't get contig stats");	
			v.push((ctg.clone(), stats));		
		}
		v.sort_unstable_by_key(|x| -(x.1.n_non_empty_bins() as i64));
		v
	}
}

struct ContigCache {
	name: String,
	contig: Option<Arc<Contig>>,	
}

pub struct ContigLookup<'a> {
	cache: Option<ContigCache>,
	contig_hash: &'a ContigHash,
}

impl <'a> ContigLookup<'a> {
	pub fn get_contig(&mut self, name: &str) -> Option<Arc<Contig>> {
		if let Some(c) = &self.cache {
			if c.name == name { 
				trace!("Got contig {} from cache", name);
				return c.contig.clone() 
			}
		}
		trace!("Looking up contig {}", name);
		let ctg = self.contig_hash.get_contig(name);
		self.cache = Some(ContigCache{name: name.to_owned(), contig: ctg.clone()});
		ctg
	}
}


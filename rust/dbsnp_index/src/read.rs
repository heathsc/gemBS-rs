use std::sync::Arc;
use std::collections::HashMap;

use super::snp::{RawSnp, Snp, SnpBlock};
pub mod read_bed;

pub struct ReaderBuf {
	buffer: HashMap<Arc<str>, Vec<RawSnp>>,	
	limit: usize,
}

impl ReaderBuf {
	pub fn new(limit: usize) -> Self {
		Self{buffer: HashMap::new(), limit}	
	}
	pub fn add_snp(&mut self, snp: Snp) {
		let (raw_snp, contig) = snp.components();
		let cname = contig.ref_name();
		let v = self.buffer.entry(cname).or_insert_with(Vec::new);	
		v.push(raw_snp);
		if v.len() >= self.limit {
			let v = self.buffer.remove(contig.name()).unwrap();
			let sb = SnpBlock::new(contig.clone(), v);
			contig.send_message(sb);
		}
	}	
}


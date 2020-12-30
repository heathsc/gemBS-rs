use std::sync::Arc;
use super::contig::*;
use super::prefix::*;

#[derive(Debug)]
pub struct RawSnp {
	name: String,
	pos: u32,
	prefix: u32,
	maf: Option<f32>,
	contig: Arc<Contig>,
}

impl RawSnp {
	pub fn name(&self) -> &str { &self.name }
	pub fn prefix(&self) -> &u32 { &self.prefix }
	pub fn pos(&self) -> u32 { self.pos }
	pub fn maf(&self) -> Option<f32> { self.maf }
	pub fn contig(&self) -> Arc<Contig> { self.contig.clone() }
}

pub struct RawSnpBuilder<'a> {
	ctg_lookup: ContigLookup<'a>,
	pref_lookup: PrefixLookup<'a>,
}

impl <'a>RawSnpBuilder<'a> {
	pub fn new(ctg_hash: &'a ContigHash, pref_hash: &'a PrefixHash) -> Self {
		Self{ctg_lookup: ctg_hash.mk_lookup(), pref_lookup: pref_hash.mk_lookup() }
	}
	pub fn build_snp(&mut self, name: &str, prefix: &str, ctg: &str, pos: u32, maf: Option<f32>) -> RawSnp {
		RawSnp {
			name: name.to_owned(),
			prefix: self.pref_lookup.get_prefix(prefix),
			contig: self.ctg_lookup.get_contig(ctg),
			pos, maf
		}	
	}
	pub fn mk_snp(&mut self, name: &str, ctg: &str, pos: u32, maf: Option<f32>) -> RawSnp {
		if let Some(ix) = name.find(char::is_numeric) {
			self.build_snp(&name[ix..], &name[0..ix], ctg, pos, maf)
		} else {
			self.build_snp("", &name, ctg, pos, maf)
		}
	}
}
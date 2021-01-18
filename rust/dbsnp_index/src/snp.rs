use std::sync::Arc;
use std::time::Duration;
use std::ops::DerefMut;
use crossbeam_channel::{Receiver, Select};

use super::contig::*;
use crate::config::Config;

#[derive(Debug)]
pub struct RawSnp {
	name: String,
	pos: u32,
	maf: Option<f32>,
}

impl RawSnp {
	pub fn name(&self) -> &str { &self.name }
	pub fn pos(&self) -> u32 { self.pos }
	pub fn maf(&self) -> Option<f32> { self.maf }
}

#[derive(Debug)]
pub struct Snp {
	raw_snp: RawSnp,
	contig: Arc<Contig>,	
}

impl Snp {
	pub fn components(self) -> (RawSnp, Arc<Contig>) {
		let Snp{raw_snp, contig} = self;
		(raw_snp, contig)
	} 
}

pub struct SnpBlock {
	contig: Arc<Contig>,
	snps: Vec<RawSnp>,
}

impl SnpBlock {
	pub fn new(contig: Arc<Contig>, snps: Vec<RawSnp>) -> Self { Self{contig, snps}}
	pub fn contig(&self) -> Arc<Contig> { self.contig.clone() }
	pub fn snps(&self) -> &[RawSnp] { &self.snps } 	
	// Get minimum and maximum positions in SnpBlock
	pub fn min_max(&self) -> Option<(u32, u32)> {
		let mut it = self.snps.iter();
		if let Some(x) = it.next() {
			Some(it.fold((x.pos(), x.pos()), |(a, b), y| (a.min(y.pos()), b.max(y.pos()))))
		} else { None }		
	}
}	

pub struct SnpBuilder<'a> {
	ctg_lookup: ContigLookup<'a>,
}

impl <'a>SnpBuilder<'a> {
	pub fn new(ctg_hash: &'a ContigHash) -> Self {
		Self{ctg_lookup: ctg_hash.mk_lookup()}
	}
	pub fn build_snp(&mut self, name: &str, ctg: &str, pos: u32, maf: Option<f32>) -> Option<Snp> {
		if let Some(contig) = self.ctg_lookup.get_contig(ctg) {
			Some(Snp {
				raw_snp: RawSnp {
					name: name.to_owned(),
					pos, maf
				},
				contig
			})
		} else { None }	
	}
	pub fn mk_snp(&mut self, name: &str, ctg: &str, pos: u32, maf: Option<f32>) -> Option<Snp> {		
		if let Some(tname) = name.strip_prefix("rs") {
			if tname.is_empty() || tname.find(|c :char| !char::is_ascii_digit(&c)).is_some() { None }
            else { self.build_snp(tname, ctg, pos, maf) }
		} else { None }
	}
}

fn store_snp_block(sb: &SnpBlock, data: &mut ContigData, conf: &Config) {
	for snp in sb.snps().iter() { data.add_snp(snp, conf); }
}

pub fn store_thread(conf: Arc<Config>, control_receiver: Receiver<bool>, thread_id: usize) {
	let mut ending = false;
	loop {	
		// Build up list of channels to watch
		let ctgs = conf.ctg_hash().get_avail_contig_list();
		let mut sel = Select::new();
		for(_, r) in ctgs.iter() { sel.recv(&r); }
		let min_max = |v: &[SnpBlock]| {
			if let Some(sb) = v.first() {
				let (x, y) = &v[1..].iter().fold(sb.min_max().unwrap(), |(a, b), s| {
					let (mn, mx) = s.min_max().unwrap();
					(a.min(mn), b.max(mx))
				});				
				Some((*x, *y))
			} else { None }
		};
		if !ending {
			let ctr_idx = sel.recv(&control_receiver);
			if let Ok(op) = sel.ready_timeout(Duration::from_millis(100)) {
				match op {
					idx if idx == ctr_idx => match control_receiver.recv() {
						Ok(_) => {
							debug!("Store thread {} received shutdown signal", thread_id);
							ending = true;
						},		
						Err(e) => panic!("Store thread {} - Error receiving message from control channel: {}", thread_id, e),
					},
					idx => {
						// Try to bind this contig
						if let Some(mut g) = ctgs[idx].0.try_bind() { 
							let v: Vec<_> = g.recv().try_iter().collect();
							if let Some((min, max)) = min_max(&v) {
								let data = g.deref_mut();
								data.check_bins(min, max);
								for sb in v.iter() {
									store_snp_block(&sb, data, conf.as_ref());
								}
							}
						}				
					},
				}	
			}			
		} else {
			let mut processed = false;
			if !ctgs.is_empty() {
				while let Ok(idx) = sel.try_ready() {
					// Try to bind this contig
					if let Some(mut g) = ctgs[idx].0.try_bind() { 
						let v: Vec<_> = g.recv().try_iter().collect();
						if let Some((min, max)) = min_max(&v) {
							let data = g.deref_mut();
							data.check_bins(min, max);
							for sb in v.iter() {
								store_snp_block(&sb, data, conf.as_ref());
								processed = true;
							}
						}
					}
				}
			}
			if !processed { break }	
		}
	}
	debug!("Store thread {} finishing up", thread_id);
	
}

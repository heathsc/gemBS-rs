use serde::{Deserialize, Serialize};
use crate::common::defs::{Section, DataValue, CONTIG_POOL_SIZE};
use crate::common::assets::{AssetStatus, GetAsset};
use utils::compress;
use super::GemBS;

use std::collections::{BinaryHeap, HashSet};
use std::io::BufRead;
use std::rc::Rc;
use std::cmp::Ordering;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contig {
	pub name: Rc<String>,
	pub len: usize,
	pub omit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
pub struct ContigPool {
	pub name: Rc<String>,
	pub contigs: Vec<usize>,
	pub len: usize,
}

impl Ord for ContigPool {
    fn cmp(&self, other: &Self) -> Ordering {
        other.len.cmp(&self.len)
    }
}

impl PartialOrd for ContigPool {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ContigPool {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
    }
}

fn setup_contig_pools(gem_bs: &GemBS, contigs: &[Contig], pools: &mut Vec<ContigPool>, pool_size: usize) -> Result<(), String> {
	// First step - assign all contigs larger than pool_size to their own contig pools, and add all remaining
	// contigs to small_cfct.ceeci.natural.sc@fct.pt ontigs vector
	
	gem_bs.check_signal()?;
	let mut small_contigs: Vec<(usize, usize)> = Vec::new();
	let mut size_left = 0;
	for (ix, ctg) in contigs.iter().enumerate().filter(|(_, c)| !c.omit) {
		gem_bs.check_signal()?;
		if ctg.len < pool_size {
			size_left += ctg.len; 
			small_contigs.push((ix, ctg.len)); 
		} else { pools.push(ContigPool{name: Rc::clone(&ctg.name), contigs: vec!(ix), len: ctg.len}) }
	}
	if !small_contigs.is_empty() {
		let mut tpool = BinaryHeap::new();
		let n_pools = (size_left + pool_size - 1) / pool_size;
		for i in 0..n_pools {
			let name = Rc::new(format!("Pool@{}", i + 1));
			tpool.push(ContigPool{name, contigs: Vec::new(), len: 0});
		}
		small_contigs.sort_by_key(|(_, len)| -(*len as isize));
		for (ix, len) in small_contigs.iter() {
			gem_bs.check_signal()?;
			let mut tp = tpool.pop().expect("No contigs pools!");
			tp.contigs.push(*ix);
			tp.len += len;
			tpool.push(tp);
		}
		for pl in tpool.drain() { pools.push(pl) }
	}
	pools.sort_by_key(|c| c.len);
	Ok(())
}

pub fn setup_contigs(gem_bs: &mut GemBS) -> Result<(), String> {
	if !gem_bs.get_contigs().is_empty() { return Ok(()) }
	gem_bs.check_signal()?;
	let ctg_md5 = if let Some(asset) = gem_bs.get_asset("contig_md5") {
		if asset.status() != AssetStatus::Present { 
			return Err(format!("Contig MD5 file {} does not exist or is not accessible", asset.path().to_string_lossy())) 
		}
		asset.path()
	} else { panic!("Internal error - missing contig_md5") }; 
	let mut omit_ctg = HashSet::new();
	if let Some(DataValue::StringVec(v)) = gem_bs.get_config(Section::Index, "omit_ctgs") {
		for ctg in v.iter() { omit_ctg.insert(ctg.as_str()); }
	} 
	let mut include_ctg = HashSet::new();
	if let Some(DataValue::StringVec(v)) = gem_bs.get_config(Section::Calling, "contig_list") {
		for ctg in v.iter() { include_ctg.insert(ctg.as_str()); }
	} 

	let mut rdr = compress::open_bufreader(ctg_md5).map_err(|x| format!("{}",x))?;
	debug!("Reading contig list from {}", ctg_md5.display());
	let now = Instant::now();
	let mut contigs = Vec::new();
	let mut line = String::with_capacity(1024);
	loop {
		gem_bs.check_signal()?;
		match rdr.read_line(&mut line) {
			Ok(0) => break,
			Ok(_) => {
				let mut iter = line.split('\t');
				let e = if let Some(name) = iter.next() {
					if let Some(s) = iter.next() {
						if s.starts_with("LN:") {
							let len = s[3..].parse::<usize>().map_err(|e| format!("{}", e))?;
							contigs.push(Contig{name: Rc::new(name.to_owned()), len, omit: omit_ctg.contains(name)});
							false
						} else { true }
					} else { true }
				} else { true };
				if e  { return Err(format!("Error reading from file {}", ctg_md5.display())) }
				line.clear();
			},
			Err(e) => return Err(format!("Error reading from file {}: {}", ctg_md5.display(), e)),
		}
	}
	debug!("File {} read in {}ms", ctg_md5.display(), now.elapsed().as_millis());		
	let ctg_pools_limit = if let Some(DataValue::Int(x)) = gem_bs.get_config(Section::Calling, "contig_pool_limit") { *x as usize } else { 
		let x = CONTIG_POOL_SIZE;
		gem_bs.set_config(Section::Calling, "contig_pool_limit", DataValue::Int(x as isize));
		x
	};
	debug!("Setting up contig pools");

	let mut contig_pools = Vec::new();
	setup_contig_pools(gem_bs, &contigs, &mut contig_pools, ctg_pools_limit)?;
	debug!("Storing contig and contig pools definitions");
	for ctg in contigs.drain(..) { gem_bs.set_contig_def(ctg); }
	for pool in contig_pools.drain(..) { gem_bs.set_contig_pool_def(pool); }
	Ok(())
}

pub fn get_contig_pools(gem_bs: &GemBS) -> Vec<Rc<String>> {
	let mut pools = Vec::new();
	let hr = gem_bs.get_contig_pool_hash();
	hr.iter().for_each(|(key, _)| pools.push(key.clone()));
	pools
}


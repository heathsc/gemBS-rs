use serde::{Deserialize, Serialize};
use crate::common::defs::{Section, DataValue, CONTIG_POOL_SIZE};
use crate::common::assets::{AssetStatus, GetAsset};
use crate::config::GemBS;
use std::collections::HashSet;
use std::io::BufRead;
use std::rc::Rc;
use blake2::{Blake2b, Digest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contig {
	pub name: Rc<String>,
	pub md5: String,
	pub len: usize,
	pub omit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContigPool {
	pub name: Rc<String>,
	pub contigs: Vec<Rc<String>>,
	pub len: usize,
}

fn setup_contig_pools(contigs: &[Contig], pools: &mut Vec<ContigPool>, pool_size: usize) -> Result<(), String> {
	// First step - assign all contigs larger than pool_size to their own contig pools, and add all remaining
	// contigs to small_cfct.ceeci.natural.sc@fct.pt ontigs vector
	
	let mut small_contigs: Vec<&Contig> = Vec::new();
	for ctg in contigs.iter().filter(|c| !c.omit) {
		if ctg.len < pool_size { small_contigs.push(ctg) }
		else { pools.push(ContigPool{name: Rc::clone(&ctg.name), contigs: vec!(Rc::clone(&ctg.name)), len: ctg.len}) }
	}
	if !small_contigs.is_empty() {
		let mut tpool = Vec::new();
		let size_left = small_contigs.iter().fold(0, |sum, c| sum + c.len);
		let n_pools = (size_left + pool_size - 1) / pool_size;
		for i in 0..n_pools {
			let name = Rc::new(format!("Pool@{}", i + 1));
			tpool.push(ContigPool{name, contigs: Vec::new(), len: 0});
		}
		small_contigs.sort_by_key(|c| -(c.len as isize));
		for ctg in small_contigs.iter() {
			tpool[0].contigs.push(Rc::clone(&ctg.name));
			tpool[0].len += ctg.len;
			tpool.sort_by_key(|c| c.len)
		}
		for pl in tpool.drain(..) {	pools.push(pl) }
	}
	pools.sort_by_key(|c| c.len);
	Ok(())
}

pub fn setup_contigs(gem_bs: &mut GemBS) -> Result<String, String> {
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

	trace!("Reading contig list from {:?}", ctg_md5);
	let rdr = compress::open_bufreader(ctg_md5).map_err(|x| format!("{}",x))?;
	let mut contigs = Vec::new();
	
	for (i, line) in rdr.lines().enumerate() {
		if let Ok(st) = line {
			let mut name_str = None;
			let mut len_str = None;
			let mut md5_str = None;
			for (ix, s) in st.split('\t').enumerate() {
				if ix == 0 { name_str = Some(s) }
				else if s.starts_with("LN:") { len_str = Some(s.trim_start_matches("LN:")) }
				else if s.starts_with("M5:") { md5_str = Some(s.trim_start_matches("M5:")) }
			}
			let (omit, name) = if let Some(s) = name_str { 
				(if omit_ctg.contains(s) { true } else if include_ctg.is_empty() { false } else { !include_ctg.contains(s) }, Rc::new(s.to_owned()))
			} else { 
				return Err(format!("Error reading contig name from file {} at line {}", ctg_md5.to_string_lossy(), i)) 
			};
			let len = if let Some(s) = len_str { s.parse::<usize>().map_err(|e| format!("{}", e))? } else { 
				return Err(format!("Error reading contig length for {} from file {} at line {}", name, ctg_md5.to_string_lossy(), i)) 
			};
			let md5 = if let Some(s) = md5_str { s.to_owned() } else { 
				return Err(format!("Error reading MD5 hash for {} from file {} at line {}", name, ctg_md5.to_string_lossy(), i)) 
			};
			contigs.push(Contig{name, md5, len, omit});
		}
	} 
	
	let ctg_pools_limit = if let Some(DataValue::Int(x)) = gem_bs.get_config(Section::Calling, "contig_pool_limit") { *x as usize } else { 
		let x = CONTIG_POOL_SIZE;
		gem_bs.set_config(Section::Calling, "contig_pool_limit", DataValue::Int(x as isize));
		x
	};

	let mut contig_pools = Vec::new();
	setup_contig_pools(&contigs, &mut contig_pools, ctg_pools_limit)?;
	
	// Construct Hash of pool names and contents
	
	// First we construct a list of pools sorted by size
	let mut tvec = contig_pools.iter().collect::<Vec<_>>();
	tvec.sort_by_key(|p| &p.name);
	// Now make the hash from a list of each pool name followed by a sorted list of contig names for that pool
	let mut hasher = Blake2b::new();
	for p in tvec {
		hasher.input(p.name.as_bytes());
		itertools::sorted(p.contigs.iter().collect::<Vec<_>>()).for_each(|x| hasher.input(x.as_bytes()));
	}
	let digest = hasher.result().iter().fold(String::new(), |mut s, x| { s.push_str(format!("{:02x}", x).as_str()); s});
	debug!("Contig pool digest = {}", digest);
	debug!("Storing contig and contig pools definitions");
	for ctg in contigs.drain(..) { gem_bs.set_contig_def(ctg); }
	for pool in contig_pools.drain(..) { gem_bs.set_contig_pool_def(pool); }
	Ok(digest)
}

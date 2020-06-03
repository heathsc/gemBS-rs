use serde::{Deserialize, Serialize};
use crate::common::defs::{Section, DataValue, ContigInfo, ContigData};
use crate::config::GemBS;
use std::collections::HashSet;
use std::path::Path;
use std::io::BufRead;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contig {
	pub name: String,
	pub md5: String,
	pub len: usize,
	pub omit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContigPool {
	pub name: String,
	pub contigs: Vec<String>,
	pub len: usize,
}

fn setup_contig_pools(contigs: &[Contig], pools: &mut Vec<ContigPool>, pool_size: usize) -> Result<(), String> {
	// First step - assign all contigs larger than pool_size to their own contig pools, and add all remaining
	// contigs to small_contigs vector
	
	let mut small_contigs: Vec<&Contig> = Vec::new();
	for ctg in contigs.iter().filter(|c| !c.omit) {
		if ctg.len < pool_size { small_contigs.push(ctg) }
		else { pools.push(ContigPool{name: ctg.name.clone(), contigs: vec!(ctg.name.clone()), len: ctg.len}) }
	}
	if !small_contigs.is_empty() {
		let mut tpool = Vec::new();
		let size_left = small_contigs.iter().fold(0, |sum, c| sum + c.len);
		let n_pools = (size_left + pool_size - 1) / pool_size;
		for i in 0..n_pools {
			let name = format!("Pool@{}", i + 1);
			tpool.push(ContigPool{name, contigs: Vec::new(), len: 0});
		}
		small_contigs.sort_by_key(|c| -(c.len as isize));
		for ctg in small_contigs.iter() {
			tpool[0].contigs.push(ctg.name.clone());
			tpool[0].len += ctg.len;
			tpool.sort_by_key(|c| c.len)
		}
		for pl in tpool.drain(..) {	pools.push(pl) }
	}
	pools.sort_by_key(|c| c.len);
	Ok(())
}

pub fn setup_contigs(gem_bs: &mut GemBS) -> Result<(), String> {
	let ctg_md5 = if let Some(DataValue::String(file)) = gem_bs.get_config(Section::Index, "ctg_md5") { 
		let path = Path::new(file);
		if !path.exists() { return Err(format!("Contig MD5 file {} does not exist or is not accessible", file)) }
		path
	} else { panic!("Internal error - missing ctg_md5") };
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
				(if omit_ctg.contains(s) { true } else if include_ctg.is_empty() { false } else { !include_ctg.contains(s) }, s.to_owned())
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
		let x: usize = 25_000_000;
		gem_bs.set_config(Section::Calling, "contig_pool_limit", DataValue::Int(x as isize));
		x
	};

	let mut contig_pools = Vec::new();
	setup_contig_pools(&contigs, &mut contig_pools, 
	ctg_pools_limit)?;
	debug!("Storing contig and contig pools definitions");
	for ctg in contigs.drain(..) { gem_bs.set_contig_def(ctg); }
	for pool in contig_pools.drain(..) { gem_bs.set_contig_pool_def(pool); }
	Ok(())
}
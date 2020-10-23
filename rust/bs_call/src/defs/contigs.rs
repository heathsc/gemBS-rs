use std::io;
use std::collections::HashSet;
use std::str::FromStr;

use utils::compress;
use crate::config::*;
use crate::htslib::{SamFile, VcfHeader, Faidx, BCF_DT_CTG};

#[derive(Default, Clone)]
pub struct CtgInfo {
	ref_id: Option<usize>,
	vcf_id: Option<usize>,
	name: String,
	in_header: bool,
} 

impl CtgInfo {
	fn new<S: AsRef<str>>(name: S) -> Self {CtgInfo { ref_id: None, vcf_id: None, in_header: false, name: name.as_ref().to_owned()}}
	pub fn in_header(&self) -> bool { self.in_header }
	pub fn vcf_id(&self) -> Option<usize> { self.vcf_id }
	pub fn ref_id(&self) -> Option<usize> { self.ref_id }
	pub fn name(&self) -> &str { &self.name }
}

#[derive(Copy, Clone)]
pub struct CtgRegion {
	pub sam_tid: usize,
	pub start: usize,
	pub stop: usize,	
}

impl CtgRegion {
	fn new(sam_tid: usize, start: usize, stop: usize) -> Self {
		if stop < start { panic!("CtgRegion::new() stop < start") }
		CtgRegion{sam_tid, start, stop}
	}	
}

fn read_omit_ctg_list(chash: &ConfHash, sam_file: &SamFile) -> io::Result<HashSet<usize>> {
	let mut omit_ctgs = HashSet::new();
	if let Some(fname) = chash.get_str("contig_exclude") {
		let mut rdr = compress::open_bufreader(fname)?;
		debug!("Reading contig exclude list from {}", fname);
		let mut line = String::with_capacity(256);
		loop {
			match rdr.read_line(&mut line) {
				Ok(0) => break,
				Ok(_) => {
					let mut iter = line.split('\t');
					if let Some(ctg) = iter.next() {
						let ctg = ctg.trim();
						if !ctg.is_empty() {
							if let Some(x) = sam_file.name2tid(ctg) { 
								trace!("Adding {} (tid = {}) to omit list", ctg, x);
								omit_ctgs.insert(x); 
							} else { warn!("Contig {} in contig exclude file {} not present in SAM header (ignored)", ctg, fname)}
						}
					}
					line.clear();
				},
				Err(e) => return Err(e),
			}
		}
		debug!("Done");
	}
	Ok(omit_ctgs)
}

fn init_sam_contigs(sam_file: &SamFile) -> Vec<CtgInfo> {
	(0..sam_file.nref()).fold(Vec::new(), |mut v, i| {v.push(CtgInfo::new(sam_file.tid2name(i))); v})	
}

fn setup_ref_ids(ctgs: &mut Vec<CtgInfo>, sam_file: &SamFile, ref_idx: &Faidx) {
	for i in 0..ref_idx.nseq() {
		let name = ref_idx.iseq(i);
		if let Some(tid) = sam_file.name2tid(name) {
			ctgs[tid].ref_id = Some(i);
			trace!("Matched Reference contig {} ({}) to SAM header sequence {}", i, name, tid);
		} 
	}
}

fn setup_regions_with_contig_bed(fname: &str, filter: bool, ctgs: &mut Vec<CtgInfo>, sam_file: &SamFile, ref_idx: &Faidx, omit_ctgs: &HashSet<usize>) -> io::Result<Vec<CtgRegion>> {
	let mut ctg_regions = Vec::new();
	// If we are not filtering then mark all contigs from SAM header that are *not* in omit_ctgs for output in VCF header
	if !filter { ctgs.iter_mut().enumerate().for_each(|(i, c)| c.in_header = !omit_ctgs.contains(&i)) }
	let mut rdr = compress::open_bufreader(fname)?;
	debug!("Reading region list from {}", fname);
	let mut line = String::with_capacity(256);
	let mut l = 0;
	loop {
		l += 1;
		match rdr.read_line(&mut line) {
			Ok(0) => break,
			Ok(_) => {
				let v: Vec<_> = line.split('\t').collect();
				if v.len() >= 3 {
					let name = v[0].trim();
					if let Some(tid) = sam_file.name2tid(name) {
						if !omit_ctgs.contains(&tid) {
							if ctgs[tid].ref_id.is_none() { return Err(new_err(format!("Contig {} missing in reference FASTA", name))); }
							if let Some(len) = ref_idx.seq_len(name) {
								if sam_file.tid2len(tid) != len { return Err(new_err(format!("Mismatch in sequence lengths between reference and SAM header for contig {}: {}, {}", name, len, sam_file.tid2len(tid)))); }
								let len = sam_file.tid2len(tid);
								let start = <usize>::from_str(v[1].trim());
								let stop = <usize>::from_str(v[2].trim());
								if let (Ok(x), Ok(y)) = (start, stop) {
									if x >= y || y > len { return Err(new_err(format!("Error parsing region list file {} at line {}: coordinates out of range", fname, l))); } 
									ctgs[tid].in_header = true;
									ctg_regions.push(CtgRegion::new(tid, x , y - 1));
									trace!("Added region {}:{}-{}", name, x + 1, y);
								} else { return Err(new_err(format!("Error parsing region list file {} at line {}: coordinates invalid", fname, l))); } 
							} else { panic!("Missing sequence length from reference index for contig {}", name); }
						}
					} else { return Err(new_err(format!("Contig {} in region list file {} not present in SAM header", name, fname))); }
				} else if !(v.len() == 1 && v[0].trim().is_empty()) {
					return Err(new_err(format!("Error parsing region list file {} at line {}", fname, l)));
				}
			},
			Err(e) => return Err(e),
		}
		line.clear();
	} 
	ctgs.iter().enumerate().filter(|(_,c)| c.in_header).for_each(|(i,_)| trace!("Contig {} will be output in VCF header", sam_file.tid2name(i)));
	debug!("Done");
	Ok(ctg_regions)	
}

fn setup_regions_from_sam_header(ctgs: &mut Vec<CtgInfo>, sam_file: &SamFile, ref_idx: &Faidx, omit_ctgs: &HashSet<usize>) -> io::Result<Vec<CtgRegion>> {
	let mut ctg_regions = Vec::new();
	for (i, c) in ctgs.iter_mut().enumerate() {
		if !omit_ctgs.contains(&i) {
			let name = sam_file.tid2name(i);
			if c.ref_id.is_some() {
				if let Some(l) = ref_idx.seq_len(name) {
					if sam_file.tid2len(i) != l { return Err(new_err(format!("Mismatch in sequence lengths between reference and SAM header for contig {}: {}, {}", name, l, sam_file.tid2len(i)))); }
					c.in_header = true;
					trace!("Contig {} will be output in VCF header", name);
					ctg_regions.push(CtgRegion::new(i, 0, l - 1));
					trace!("Added region {}:{}-{}", name, 1, l);
				} else { panic!("Missing sequence length from reference index for contig {}", name); }
			} else { return Err(new_err(format!("Contig {} missing in reference FASTA", name))); } 
		}
	}
	Ok(ctg_regions)
}

pub fn setup_contigs(chash: &ConfHash, sam_file: &SamFile, ref_idx: &Faidx) -> io::Result<(Vec<CtgInfo>, Vec<CtgRegion>)> {
	let omit_ctgs = read_omit_ctg_list(chash, sam_file)?;
	let mut ctgs = init_sam_contigs(sam_file);
	setup_ref_ids(&mut ctgs, sam_file, ref_idx);
	let ctg_regions = if let Some(fname) = chash.get_str("contig_bed") {
		let filter = chash.get_bool("filter_contigs");
		setup_regions_with_contig_bed(fname, filter, &mut ctgs, sam_file, ref_idx, &omit_ctgs)?
	} else {
		setup_regions_from_sam_header(&mut ctgs, sam_file, ref_idx, &omit_ctgs)?
	};
	
	Ok((ctgs, ctg_regions))
}

pub fn set_contig_vcf_ids(vcf_hdr: &VcfHeader, ctgs: &mut [CtgInfo], sam_file: &SamFile) {
	let mut vcf_ids = Vec::with_capacity(sam_file.nref());
	for tid in 0..sam_file.nref() { 
		let c = sam_file.tid2name(tid);
		vcf_ids.push(vcf_hdr.id2int(BCF_DT_CTG as usize, c));
	}
	for (tid, vid) in vcf_ids.iter().enumerate() {
		if ctgs[tid].in_header {
			if vid.is_none() { panic!("Contig not found in BCF/VCF header"); }
			ctgs[tid].vcf_id = *vid;
		}
	}
}

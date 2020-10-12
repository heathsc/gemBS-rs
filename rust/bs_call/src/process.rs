use std::io;

use crate::htslib::*;
use crate::config::BsCallConfig;
use crate::records::{ReadEnd, MFLAG_LAST};

pub mod vcf;
pub mod sam;
pub use vcf::*;
pub use sam::*;

pub fn process(bs_cfg: &mut BsCallConfig) -> io::Result<()> {
	bs_cfg.sam_input.set_region_itr(&bs_cfg.regions)?;
	let brec = BamRec::new().unwrap();
	loop {
		match bs_cfg.sam_input.get_next(&brec) {
			SamReadResult::Ok => (),
			SamReadResult::EOF => break,
			_ => panic!("Error reading record"),
		} 
		let (read_end, read_flag) = ReadEnd::from_bam_rec(&bs_cfg.conf_hash, &bs_cfg.sam_input.hdr, &brec);
		if let Some(read) = read_end {
			let map = &read.maps[0];
			let ctg = bs_cfg.sam_input.tid2name(map.map_pos.tid as usize);
			let bs_strand = map.bs_strand();
			let sq = &read.seq_qual;
			let last = (read.maps[0].flags & MFLAG_LAST) != 0;
			println!("{}\t{}\t{}\t{:?}\t{:?}\t{}\t{}\t{}\t{:#}", read.id, ctg, map.map_pos.pos + 1, bs_strand, read_flag, last, read.maps.len(), sq, sq);
			
		} else {
			println!("{}\t{:?}", brec.qname(), read_flag);
		}
	}
	Ok(())
}
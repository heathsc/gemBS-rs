use std::io;

use crate::htslib::*;
use crate::config::BsCallConfig;

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
		let ctg = if let Some(tid) = brec.tid() {
			bs_cfg.sam_input.tid2name(tid)
		} else { "*" };
		let bs_strand = get_bs_strand(&brec);
		let sq = brec.get_seq_qual().unwrap();
		println!("{}\t{}\t{}\t{:?}\t{}\t{:#}", brec.qname(), ctg, brec.pos().unwrap() + 1, bs_strand, sq, sq);
	}
	Ok(())
}
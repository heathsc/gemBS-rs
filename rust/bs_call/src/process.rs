use std::io;

use crate::config::BsCallConfig;

pub mod vcf;
pub mod sam;
pub use vcf::*;
pub use sam::*;

pub fn process(bs_cfg: &BsCallConfig) -> io::Result<()> {
	let itr = bs_cfg.sam_input.region_iter()?;
	for _ in itr {
		
	}
	Ok(())
}
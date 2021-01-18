use std::str::FromStr;

use super::*;
use crate::snp::SnpBuilder;
use crate::config::Config;

fn snp_from_vcf(s: &str, rb: &mut SnpBuilder) -> Option<Snp> {

	None	
}

pub fn process_vcf_line(buf: &str, builder: &mut SnpBuilder, rbuf: &mut ReaderBuf) {
	if let Some(snp) = snp_from_vcf(&buf, builder) { rbuf.add_snp(snp) }	
}

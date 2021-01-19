use std::str::FromStr;

use super::*;
use crate::snp::SnpBuilder;
use crate::config::Config;

fn snp_from_vcf(s: &str, rb: &mut SnpBuilder) -> Option<Snp> {
	let v: Vec<&str> = s.split('\t').collect();
	if v.len() > 4 && v[3].len() == 1 && v[4].len() == 1 {
		let pos = <u32>::from_str(&v[1]).ok()?;
		return rb.mk_snp(v[2], v[0], pos, None)
	}
	None	
}

pub fn process_vcf_line(buf: &str, builder: &mut SnpBuilder, rbuf: &mut ReaderBuf) {
	if !buf.starts_with('#') {
		if let Some(snp) = snp_from_vcf(&buf, builder) { rbuf.add_snp(snp) }
	}	
}

use std::str::FromStr;
// use std::io;

use super::*;
use crate::snp::SnpBuilder;
use crate::config::Config;

// use crate::process::AtomicServer;
// use utils::compress::get_reader;

fn snp_from_bed(s: &str, rb: &mut SnpBuilder) -> Option<Snp> {
	let v: Vec<&str> = s.split('\t').collect();
	if v.len() > 4 {
		let x = <u32>::from_str(&v[1]).ok()?;
		let y = <u32>::from_str(&v[2]).ok()?;
		if y > x && y - x == 1 { return rb.mk_snp(v[3], v[0], y, None)}
	}
	None	
}

pub fn process_bed_line(conf: &Config, buf: &str, builder: &mut SnpBuilder, rbuf: &mut ReaderBuf) {
	if buf.starts_with("track") { conf.cond_set_description(&buf); }
	else if let Some(snp) = snp_from_bed(&buf, builder) { rbuf.add_snp(snp) }	
}

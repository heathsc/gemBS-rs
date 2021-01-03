use std::str::FromStr;
use super::*;

pub fn snp_from_bed(s: &str, rb: &mut SnpBuilder) -> Option<Snp> {
	let v: Vec<&str> = s.split('\t').collect();
	if v.len() > 4 {
		let x = <u32>::from_str(&v[1]).ok()?;
		let y = <u32>::from_str(&v[2]).ok()?;
		if y > x && y - x == 1 { return Some(rb.mk_snp(v[3], v[0], y, None))}
	}
	None	
}
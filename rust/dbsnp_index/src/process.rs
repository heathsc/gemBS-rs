use std::io;

use crate::config::*;
use super::contig::*;
use super::prefix::*;
use super::snp::*;

pub fn process(conf: &Config, files: &Box<[String]>) -> io::Result<()> {
	
	let ctg_hash = ContigHash::new();
	let pref_hash = PrefixHash::new();
	let mut builder = RawSnpBuilder::new(&ctg_hash, &pref_hash);
	let snp1 = builder.mk_snp("rs365453", "chr1", 145043, None);
	let snp2 = builder.mk_snp("rs7364323", "chr1", 145048, None);
	let snp3 = builder.mk_snp("ss365453", "chr2", 5043, None);
	let snp4 = builder.mk_snp("rs254", "chr1", 185043, None);
	println!("{:?}\n{:?}\n{:?}\n{:?}", snp1, snp2, snp3, snp4);
	Ok(())	
}
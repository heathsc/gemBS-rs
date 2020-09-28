use std::io;

use crate::htslib;
use crate::config::*;

pub mod write_header;
pub use write_header::write_vcf_header;

pub fn open_vcf_output(output: Option<&str>, otype: OType) -> io::Result<htslib::VcfFile> {
	debug!("Opening output file");
	let out_name = output.unwrap_or("-");
	let mode = format!("{}", otype);
	htslib::VcfFile::new(out_name, &mode)	
}
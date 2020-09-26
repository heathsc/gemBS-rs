use std::io;
use std::path::Path;

use crate::config::*;
use crate::htslib;

pub fn handle_reference(rf: &str, in_file: &mut htslib::HtsFile) -> io::Result<htslib::Faidx> {
	if !Path::new(rf).exists() { return Err(new_err(format!("Couldn't access reference file {}", rf))); }
	let fai = format!("{}.fai", rf);
	if !Path::new(&fai).exists() { return Err(new_err(format!("Couldn't access reference file index {}", fai))); }
	debug!("Trying to open index for reference {}", rf);
	let idx = htslib::faidx_load(rf)?;
	in_file.set_fai_filename(&fai)?;
	debug!("Index loaded");
	Ok(idx) 
}

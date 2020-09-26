use std::io;

use crate::htslib;

pub fn open_sam_input(fname: Option<&str>) -> io::Result<(htslib::HtsFile, htslib::HtsIndex, htslib::SamHeader)> {
	let rname = fname.unwrap_or("<STDIN>");
	debug!("Opening input file");
	let mut in_file = htslib::hts_open(rname, "r")?;
	debug!("Trying to load index for input {}", rname);
	let idx = in_file.sam_index_load()?;
	debug!("Index loaded");
	debug!("Trying to read header from {}", rname);
	let hdr = in_file.read_header()?;
	debug!("Header loaded");
	Ok((in_file, idx, hdr))
}

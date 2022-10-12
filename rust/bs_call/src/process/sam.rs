use std::io;

use crate::htslib;

pub fn open_sam_input(fname: Option<&str>) -> io::Result<htslib::SamFile> {
    let rname = fname.unwrap_or("-");
    debug!("Opening input file");
    htslib::SamFile::new(rname)
}

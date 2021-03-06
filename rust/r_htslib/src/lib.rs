use std::ffi::{CStr, CString};
use std::io;
use std::io::{Error, ErrorKind};

pub mod hts;
pub use hts::*;
pub mod sam;
pub use sam::*;
pub mod vcf;
pub use vcf::*;
pub mod tbx;
pub use tbx::*;
pub mod bgzf;
pub use bgzf::*;
pub mod faidx;
pub use faidx::*;
pub mod kstring;
pub use kstring::*;
pub mod synced_bcf_reader;
pub use synced_bcf_reader::*;

pub mod malloc_data_block;
pub use malloc_data_block::*;

pub fn get_cstr<S: AsRef<str>>(s: S) -> CString { CString::new(s.as_ref().as_bytes()).unwrap() }

fn from_cstr<'a>(cstr: *const i8) -> &'a str {
	if cstr.is_null() { panic!("from_cstr() called with a NULL"); }
	unsafe { CStr::from_ptr(cstr).to_str().expect("C String not UTF8") }
}

pub fn hts_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

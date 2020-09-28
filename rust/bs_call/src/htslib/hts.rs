use std::io;

use super::{SamHeader, get_cstr};

use crate::config::new_err;
use rust_htslib::htslib;

pub struct HtsFile {
	inner: *mut htslib::htsFile,
	name: String,
}

impl Drop for HtsFile {
	fn drop(&mut self) {
		unsafe { htslib::hts_close(self.inner) };
	}
}

impl HtsFile {
	pub fn new<S: AsRef<str>>(name: S, mode: &str) -> io::Result<Self> {
		let name = name.as_ref();
		let fptr = unsafe{ htslib::hts_open(get_cstr(name).as_ptr(), get_cstr(mode).as_ptr()) };
		if fptr.is_null() {	Err(new_err(format!("Couldn't open file {} with mode {}", name, mode))) }
		else { Ok(HtsFile{inner: fptr, name: name.to_owned()}) }
	}
	pub fn sam_index_load(&mut self) -> io::Result<HtsIndex> {
		let idx = unsafe { htslib::sam_index_load(self.inner, get_cstr(&self.name).as_ptr()) };
		if idx.is_null() { Err(new_err(format!("Couldn't load SAM index for file {}", self.name))) }
		else { Ok(HtsIndex{inner: idx}) }
	}
	pub fn set_fai_filename<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> {
		let name = name.as_ref();
		let ret = unsafe { htslib::hts_set_fai_filename(self.inner, get_cstr(name).as_ptr()) };
		if ret != 0 { Err(new_err(format!("Failed to attach reference index {} to file {}", name, self.name))) } 
		else { Ok(()) }
	}
	pub fn format(&self) -> HtsFormat {	HtsFormat{inner: unsafe{htslib::hts_get_format(self.inner)}} }
	pub fn set_threads(&mut self, t: usize) -> io::Result<()> {
		let ret = unsafe{htslib::hts_set_threads(self.inner, t as libc::c_int)};
		if ret != 0 { Err(new_err(format!("Failed to set additional threads to file {}", self.name))) } 
		else { 
			debug!("Set additional threads for file {}: {}", self.name, t);
			Ok(()) 
		}
	}
	pub fn inner(&mut self) -> *mut htslib::htsFile { self.inner }
}

pub struct HtsIndex {
	inner: *const htslib::hts_idx_t,	
}

impl HtsIndex {
	pub fn inner(&self) -> *const htslib::hts_idx_t { self.inner }	
}

pub struct HtsFormat {
	inner: *const htslib::htsFormat,	
}

impl HtsFormat {
	pub fn is_compressed(&self) -> bool {
		let format = unsafe { *self.inner };
		format.compression == htslib::htsCompression_bgzf || format.format == htslib::htsExactFormat_cram
	}	
}

pub fn hts_open<S: AsRef<str>>(name: S, mode: &str) -> io::Result<HtsFile> {
	let name = name.as_ref();
	let fptr = unsafe{ htslib::hts_open(get_cstr(name).as_ptr(), get_cstr(mode).as_ptr()) };
	if fptr.is_null() {	Err(new_err(format!("Couldn't open file {} with mode {}", name, mode))) }
	else { Ok(HtsFile{inner: fptr, name: name.to_owned()}) }
}



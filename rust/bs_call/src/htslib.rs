use std::{io, ffi};

use rust_htslib::htslib;
pub use rust_htslib::htslib::{FT_VCF, FT_BCF, FT_VCF_GZ, FT_BCF_GZ, FT_GZ, FT_UNKN};
use crate::config::new_err;

fn get_cstr<S: AsRef<str>>(s: S) -> ffi::CString { ffi::CString::new(s.as_ref().as_bytes()).unwrap() }

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
	pub fn sam_index_load(&mut self) -> io::Result<HtsIndex> {
		let idx = unsafe { htslib::sam_index_load(self.inner, get_cstr(&self.name).as_ptr()) };
		if idx.is_null() { Err(new_err(format!("Couldn't load SAM index for file {}", self.name))) }
		else { Ok(HtsIndex{inner: idx}) }
	}
	pub fn read_header(&self) -> io::Result<SamHeader> {
		let hdr = unsafe { htslib::sam_hdr_read(self.inner) };
		if hdr.is_null() { Err(new_err(format!("Failed to load header from {}", self.name))) }
		else { Ok(SamHeader{inner: hdr}) }
	}
	pub fn format(&self) -> HtsFormat {
		HtsFormat{inner: unsafe{htslib::hts_get_format(self.inner)}}
	}
	pub fn set_fai_filename<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> {
		let name = name.as_ref();
		let ret = unsafe { htslib::hts_set_fai_filename(self.inner, get_cstr(name).as_ptr()) };
		if ret != 0 { Err(new_err(format!("Failed to attach reference index {} to file {}", name, self.name))) } 
		else { Ok(()) }
	}
	pub fn set_threads(&mut self, t: usize) -> io::Result<()> {
		let ret = unsafe{htslib::hts_set_threads(self.inner, t as libc::c_int)};
		if ret != 0 { Err(new_err(format!("Failed to set additional threads to file {}", self.name))) } 
		else { 
			debug!("Set additional threads for file {}: {}", self.name, t);
			Ok(()) 
		}
	}
}

pub struct HtsIndex {
	inner: *const htslib::hts_idx_t,	
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

#[repr(C)]
pub struct faidx_t {
    _unused: [u8; 0],
}

extern "C" {
    fn fai_load(fn_: *const ::std::os::raw::c_char) -> *mut faidx_t;
    fn faidx_nseq(fai_ : *const faidx_t) -> libc::c_int;
	fn faidx_iseq(fai_ : *const faidx_t, n_ : libc::c_int) -> *const ::std::os::raw::c_char;
    fn faidx_seq_len(fai_ : *const faidx_t, seq_: *const ::std::os::raw::c_char) -> libc::c_int;
}

pub struct Faidx {
	inner: *const faidx_t,
}

impl Faidx {
	pub fn nseq(&self) -> usize {
		let l = unsafe{ faidx_nseq(self.inner)};
		l as usize
	}
	pub fn iseq(&self, i: usize) -> &str {
		if i > self.nseq() { panic!("Sequence ID {} out of range", i); }
		let cstr = unsafe { faidx_iseq(self.inner, i as libc::c_int) };
		if cstr.is_null() { panic!("faidx_iseq() returned NULL"); }
		unsafe { ffi::CStr::from_ptr(cstr).to_str().expect("Sequence name not UTF8") }
	}	
	
	pub fn seq_len<S: AsRef<str>>(&self, cname: S) -> Option<usize> {
		let cname = cname.as_ref();
		let len = unsafe{ faidx_seq_len(self.inner, get_cstr(cname).as_ptr())};
		if len < 0 { None } else { Some(len as usize) }
	}

}

pub struct SamHeader {
	inner: *mut htslib::sam_hdr_t,	
}

impl Drop for SamHeader {
	fn drop(&mut self) {
		unsafe { htslib::sam_hdr_destroy(self.inner) };
	}
}

impl SamHeader {
	pub fn nref(&self) -> usize { 
		let l = unsafe { htslib::sam_hdr_nref(self.inner) };
		l as usize
	}
	
	fn check_idx(&self, i: usize) { if i >= self.nref() { panic!("Reference ID {} out of range", i); }}
	
	pub fn tid2name(&self, i: usize) -> &str {
		self.check_idx(i);
		let cstr = unsafe { htslib::sam_hdr_tid2name(self.inner, i as libc::c_int) };
		if cstr.is_null() { panic!("sam_hdr_tid2name() returned NULL"); }
		unsafe { ffi::CStr::from_ptr(cstr).to_str().expect("Reference name not UTF8") }
	}
	
	pub fn tid2len(&self, i: usize) -> usize {
		self.check_idx(i);
		let len = unsafe { htslib::sam_hdr_tid2len(self.inner, i as libc::c_int) };
		len as usize
	}
	
	pub fn name2tid<S: AsRef<str>>(&self, cname: S) -> Option<usize> {
		let cname = cname.as_ref();
		let tid = unsafe{ htslib::sam_hdr_name2tid(self.inner, get_cstr(cname).as_ptr())};
		if tid < 0 { None } else { Some(tid as usize) }
	}
}

pub fn hts_open<S: AsRef<str>>(name: S, mode: &str) -> io::Result<HtsFile> {
	let name = name.as_ref();
	let fptr = unsafe{ htslib::hts_open(get_cstr(name).as_ptr(), get_cstr(mode).as_ptr()) };
	if fptr.is_null() {	Err(new_err(format!("Couldn't open file {} with mode {}", name, mode))) }
	else { Ok(HtsFile{inner: fptr, name: name.to_owned()}) }
}

pub fn faidx_load<S: AsRef<str>>(name: S) -> io::Result<Faidx> {
	let name = name.as_ref();
	let idx = unsafe{ fai_load(get_cstr(name).as_ptr()) };
	if idx.is_null() { Err(new_err(format!("Failed to load reference file index {}", name))) }
	else { Ok(Faidx{inner: idx}) }
}

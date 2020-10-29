use std::ptr::NonNull;
use std::io;

use libc::{c_char, c_int, c_void, free};
use super::{hts_err, get_cstr, from_cstr, HtsPos};

#[repr(C)]
pub struct faidx_t {
    _unused: [u8; 0],
}

extern "C" {
    fn fai_load(fn_: *const c_char) -> *mut faidx_t;
    fn faidx_nseq(fai : *const faidx_t) -> c_int;
	fn faidx_iseq(fai : *const faidx_t, n : c_int) -> *const c_char;
    fn faidx_seq_len(fai : *const faidx_t, seq: *const c_char) -> c_int;
	fn faidx_fetch_seq64(fai: *const faidx_t, cname: *const c_char, x: HtsPos, y: HtsPos, len: *mut HtsPos) -> *mut c_char;
}

pub struct Faidx {
	inner: NonNull<faidx_t>,
}

unsafe impl Sync for Faidx {}
unsafe impl Send for Faidx {}

impl Faidx {
	fn inner(&self) -> &faidx_t { unsafe{self.inner.as_ref()} }
	pub fn nseq(&self) -> usize {
		let l = unsafe{ faidx_nseq(self.inner())};
		l as usize
	}
	pub fn iseq(&self, i: usize) -> &str {
		if i > self.nseq() { panic!("Sequence ID {} out of range", i); }
		from_cstr(unsafe { faidx_iseq(self.inner(), i as libc::c_int) })
	}	
	
	pub fn seq_len<S: AsRef<str>>(&self, cname: S) -> Option<usize> {
		let cname = cname.as_ref();
		let len = unsafe{ faidx_seq_len(self.inner(), get_cstr(cname).as_ptr())};
		if len < 0 { None } else { Some(len as usize) }
	}
	
	pub fn fetch_seq<S: AsRef<str>>(&self, cname: S) -> io::Result<Sequence> {
		let cname = cname.as_ref();
		if let Some(slen) = self.seq_len(cname) { 
			if slen == 0 { Err(hts_err(format!("Sequence {} has zero length", cname))) }
			else {
				let mut len: HtsPos = 0;
				let seq = unsafe{ faidx_fetch_seq64(self.inner(), get_cstr(cname).as_ptr(), 0, (slen - 1) as HtsPos, &mut len) };
				if len == -2 { Err(hts_err(format!("Sequence {} not found", cname))) }
				else if len < 0 || seq.is_null() { Err(hts_err(format!("Loading of sequence data for {} failed", cname))) }
				else { Ok(Sequence{inner: NonNull::new(seq as *mut u8).unwrap(), len: len as usize, cname: cname.to_owned()}) }
			}
		} else { Err(hts_err(format!("Sequence {} not found", cname))) }
	}
}

pub fn faidx_load<S: AsRef<str>>(name: S) -> io::Result<Faidx> {
	let name = name.as_ref();
	match NonNull::new(unsafe{ fai_load(get_cstr(name).as_ptr())}) {	
		None => Err(hts_err(format!("Failed to load reference file index {}", name))),
		Some(idx) => Ok(Faidx{inner: idx}),
	 }
}

pub struct Sequence {
	inner: NonNull<u8>,
	len: usize,
	cname: String,
}

impl Drop for Sequence {
	fn drop(&mut self) { unsafe { free(self.inner.as_ptr() as *mut c_void) } }
}

impl Sequence {
	pub fn get_seq(&self, x: usize, y: usize) -> io::Result<&[u8]> {
		if x > y { Err(hts_err("Invalid coordinates (start > end)".to_string()))} 
		else {
			let y = if y >= self.len { self.len - 1 } else { y };
			let slice = unsafe { std::slice::from_raw_parts(self.inner.as_ptr(), self.len) };
			Ok(&slice[x..=y])
		}
	}
	pub fn len(&self) -> usize { self.len }
	pub fn cname(&self) -> &str { &self.cname } 
	pub fn is_empty(&self) -> bool { self.len() == 0 }
}

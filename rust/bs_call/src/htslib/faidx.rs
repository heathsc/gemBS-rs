use std::io;

use super::{get_cstr, from_cstr};
use crate::config::new_err;

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
		from_cstr(unsafe { faidx_iseq(self.inner, i as libc::c_int) })
	}	
	
	pub fn seq_len<S: AsRef<str>>(&self, cname: S) -> Option<usize> {
		let cname = cname.as_ref();
		let len = unsafe{ faidx_seq_len(self.inner, get_cstr(cname).as_ptr())};
		if len < 0 { None } else { Some(len as usize) }
	}

}

pub fn faidx_load<S: AsRef<str>>(name: S) -> io::Result<Faidx> {
	let name = name.as_ref();
	let idx = unsafe{ fai_load(get_cstr(name).as_ptr()) };
	if idx.is_null() { Err(new_err(format!("Failed to load reference file index {}", name))) }
	else { Ok(Faidx{inner: idx}) }
}

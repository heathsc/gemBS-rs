use std::io;
use std::ptr::{NonNull, null};
use std::ffi::{c_void, CStr};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use libc::{c_char, c_int};
use super::{hts_err, get_cstr, hts_idx_t, hts_itr_t, HtsItr, HtsPos, BGZF};

pub const TBX_GENERIC: i32 = 0;
pub const TBX_SAM: i32 = 1;
pub const TBX_VCF: i32 = 2;
pub const TBX_UCSC: i32 = 0x10000;

#[repr(C)]
pub struct tbx_conf_t {
	preset: i32,
	sc: i32,
	bc: i32,
	ec: i32,
	meta_char: i32,
	line_skip: i32,
}

#[allow(non_upper_case_globals)]
pub const tbx_conf_vcf: tbx_conf_t = tbx_conf_t{ preset: TBX_VCF, sc: 1, bc: 2, ec: 0, meta_char: b'#' as i32, line_skip: 0};

#[repr(C)]
pub struct tbx_t {
	conf: tbx_conf_t,
	index: *const hts_idx_t,
	dict: *mut c_void,
}

#[link(name = "hts")]
extern "C" {
	fn hts_itr_query(idx: *const hts_idx_t, tid: c_int, beg: HtsPos, end: HtsPos,
		readrec: unsafe extern "C" fn (*mut BGZF, *mut c_void, *mut c_void, *mut c_int, *mut HtsPos, *mut HtsPos) -> c_int) -> *mut hts_itr_t;
	fn tbx_index_load3(fname: *const c_char, fnidx: *const c_char, flags: c_int) -> *mut tbx_t;
	fn tbx_seqnames(tbx: *const tbx_t, n: *mut c_int) -> *mut *const c_char;
	fn tbx_readrec(fp: *mut BGZF, tbxv: *mut c_void, sv: *mut c_void, tid: *mut c_int, beg: *mut HtsPos, end: *mut HtsPos) -> c_int;
	fn tbx_destroy(tbx: *mut tbx_t);
	pub fn tbx_index_build(fname: *const c_char, min_shift: c_int, conf: *const tbx_conf_t) -> c_int;
}

impl tbx_t {
	pub fn seq_names(&self) -> Option<Vec<&str>> {
		let mut n_seq: c_int = 0;
		let p = unsafe{tbx_seqnames(self, &mut n_seq as *mut c_int)};
		if p.is_null() {
			None	
		} else {
			let mut v = Vec::with_capacity(n_seq as usize);
			for i in 0..n_seq {
				let c_str: &CStr = unsafe { CStr::from_ptr(*p.offset(i as isize)) };
    			let str_slice: &str = c_str.to_str().unwrap();
				v.push(str_slice);
			}
			unsafe {libc::free(p as *mut c_void)};
			Some(v)
		}
		
	}
	pub fn tbx_itr_queryi(&self, tid: c_int, beg: HtsPos, end: HtsPos) -> io::Result<HtsItr> {
		HtsItr::new(unsafe {hts_itr_query(self.index, tid, beg, end, tbx_readrec)}).ok_or_else(|| hts_err("Failed to obtain tbx iterator".to_string()))
	}
}

pub struct Tbx {
	inner: NonNull<tbx_t>,
	phantom: PhantomData<tbx_t>,
} 

impl Deref for Tbx {
	type Target = tbx_t;
	#[inline]
	fn deref(&self) -> &tbx_t { unsafe{self.inner.as_ref()} }	
}

impl DerefMut for Tbx {
	#[inline]
	fn deref_mut(&mut self) -> &mut tbx_t {unsafe{ self.inner.as_mut() }}
}

impl AsRef<tbx_t> for Tbx {
	#[inline]
	fn as_ref(&self) -> &tbx_t { self}	
}

impl AsMut<tbx_t> for Tbx {
	#[inline]
	fn as_mut(&mut self) -> &mut tbx_t { self}	
}

unsafe impl Sync for Tbx {}
unsafe impl Send for Tbx {}

impl Drop for Tbx {
	fn drop(&mut self) {
		unsafe { tbx_destroy(self.as_mut()) };
	}
}


impl Tbx {
	pub fn new<S: AsRef<str>>(name: S) -> io::Result<Self> {
		let name = name.as_ref();
		match NonNull::new(unsafe{ tbx_index_load3(get_cstr(name).as_ptr(), null::<c_char>(), 0)}) {
			None =>	Err(hts_err(format!("Couldn't open tabix index for file {}", name))),
			Some(p) => Ok(Tbx{inner: p, phantom: PhantomData}), 
		}
	}
}

pub enum TbxReadResult { 
	Ok,
	EOF,	
	Error,
}


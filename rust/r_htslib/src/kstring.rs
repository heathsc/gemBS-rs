use std::ptr::null_mut;
use std::convert::TryInto;
use std::marker::PhantomData;

use libc::{c_char, c_int, c_float, c_void, size_t};
use super::*;

#[repr(C)] 
pub struct kstring_t {
	l: size_t,
	m: size_t,
	s: *mut c_char,
	phantom: PhantomData<c_char>,
}

#[link(name = "hts")]
extern "C" {
	fn bcf_enc_vchar(s: *mut kstring_t, l: c_int, a: *const c_char) -> c_int;
	fn bcf_enc_vint(s: *mut kstring_t, n: c_int, a: *const i32, wsize: c_int) -> c_int;
	fn bcf_enc_vfloat(s: *mut kstring_t, l: c_int, a: *const c_float) -> c_int;
}

impl Default for kstring_t { fn default() -> Self { Self::new() }}
impl Drop for kstring_t {
	fn drop(&mut self) { self.dealloc() }	
}

impl kstring_t {
	pub fn new() -> Self {Self{l:0, m:0, s: null_mut::<c_char>(), phantom: PhantomData}}
	pub fn initialize(&mut self) {
		self.l = 0;
		self.m = 0;
		self.s = null_mut::<c_char>();
	}
	fn dealloc(&mut self) { if !self.s.is_null() { unsafe{libc::free(self.s as *mut c_void)} } }
	pub fn free(&mut self) { 
		self.dealloc();	
		self.initialize();	
	}
	pub fn to_str(&self) -> Option<&str> {
		if self.s.is_null() || self.l == 0 { None }
		else {
			match std::str::from_utf8(unsafe {std::slice::from_raw_parts(self.s as *const u8, self.l as usize)} ) {
				Ok(s) => Some(s),
				Err(_) => None,
			}
		}
	}
	pub fn to_u8(&self) -> Option<&[u8]> {
		if self.s.is_null() { None }
		else {Some(unsafe {std::slice::from_raw_parts(self.s as *const u8, self.l as usize )})}
	}
	pub fn clear(&mut self) { self.l = 0 }
	pub fn resize(&mut self, size: size_t) -> bool {
		if self.m < size {
			let size = if size > (usize::MAX >> 2) { size } else { size + (size >> 1) };
			let tmp = unsafe { libc::realloc(self.s as *mut c_void, size) };
			if tmp.is_null() { return true }
			self.s = tmp as *mut c_char;
			self.m = size;
		}
		false
	}
//	fn expand(&mut self, expansion: size_t) -> bool {
//		let new_size = self.l + expansion;
//		if new_size < self.l { true } else { self.resize(new_size) }
//	}
	pub fn putsn(&mut self, p: *const c_char, l: size_t) -> bool {
		let new_sz = self.l + l + 2;
		if new_sz <= self.l || self.resize(new_sz) { true }
		else {
			let l1 = self.l as isize;
			unsafe {
				libc::memcpy(self.s.offset(l1) as *mut c_void, p as *const c_void, l); 
				self.l += l;
				*(self.s.offset(l1 + l as isize)) = 0; 
			}
			false
		}
	}
	pub fn putsn_(&mut self, p: *const c_char, l: size_t) -> bool {
		let new_sz = self.l + l + 1;
		if new_sz <= self.l || self.resize(new_sz) { true }
		else {
			unsafe { libc::memcpy(self.s.offset(self.l.try_into().unwrap()) as *mut c_void, p as *const c_void, l); }
			self.l += l;
			false
		}
	}
	pub fn putc(&mut self, c: c_char) -> bool {
		if self.resize(self.l + 2) { true }
		else {
			let l = self.l as isize;
			unsafe {
				*(self.s.offset(l)) = c; 
				*(self.s.offset(l + 1)) = 0; 
			}
			self.l += 1;
			false
		}
	}
	pub fn putc_(&mut self, c: c_char) -> bool {
		if self.resize(self.l + 1) { true }
		else {
			let l = self.l as isize;
			unsafe { *(self.s.offset(l)) = c; }
			self.l += 1;
			false
		}
	}
	pub fn bcf_enc_size(&mut self, size: c_int, bcf_type: u8) -> bool {
		if size >= 15 {
			self.putc((15 << 4 | bcf_type) as c_char) || if size >= 128 {
				if size >= 32768 {
					self.putc((1 << 4 | BCF_BT_INT32) as c_char) || self.putsn((size as c_int).to_le_bytes().as_ptr() as *const c_char, 4)
				} else {
					self.putc((1 << 4 | BCF_BT_INT16) as c_char) || self.putsn((size as u16).to_le_bytes().as_ptr() as *const c_char, 2)					
				}
			} else { self.putc((1 << 4 | BCF_BT_INT8) as c_char) || self.putc(size as c_char) }
		} else { self.putc(((size as u8) << 4 | bcf_type) as c_char) }
	}
	pub fn bcf_enc_int1(&mut self, x: c_int) -> bool {
		if x == bcf_int32_vector_end {
			self.bcf_enc_size(1, BCF_BT_INT8) || self.putc(bcf_int8_vector_end as c_char)
		} else if x == bcf_int32_missing {
			self.bcf_enc_size(1, BCF_BT_INT8) || self.putc(bcf_int8_missing as c_char)
		} else if x <= BCF_MAX_BT_INT8 && x >= BCF_MIN_BT_INT8 {
			self.bcf_enc_size(1, BCF_BT_INT8) || self.putc(x as c_char)
		} else if x <= BCF_MAX_BT_INT16 && x >= BCF_MIN_BT_INT16 {
			self.bcf_enc_size(1, BCF_BT_INT16) || self.putsn((x as u16).to_le_bytes().as_ptr() as *const i8, 2)							
		} else {
			self.bcf_enc_size(1, BCF_BT_INT32) || self.putsn(x.to_le_bytes().as_ptr() as *const i8, 4)				
		}	
	}
	pub fn bcf_enc_vchar(&mut self, v: &[u8]) -> bool { 
		unsafe { bcf_enc_vchar(self as *mut kstring_t, v.len() as c_int, v.as_ptr() as *const c_char) < 0 } 
	}
	pub fn bcf_enc_vint(&mut self, v: &[i32]) -> bool { 
		unsafe { bcf_enc_vint(self as *mut kstring_t, v.len() as c_int, v.as_ptr(), -1) < 0 } 
	}
	pub fn bcf_enc_vfloat(&mut self, v: &[f32]) -> bool { 
		unsafe { bcf_enc_vfloat(self as *mut kstring_t, v.len() as c_int, v.as_ptr()) < 0 } 
	}
}

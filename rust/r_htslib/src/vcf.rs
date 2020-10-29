use std::io;
use std::ptr::NonNull;

use c2rust_bitfields::BitfieldStruct;
use libc::{c_char, c_int};
use super::{hts_err, get_cstr, from_cstr, htsFile, HtsFile, HtsPos, kstring_t};

pub const BCF_DT_ID: u32 = 0;
pub const BCF_DT_CTG: u32 = 1;
pub const BCF_DT_SAMPLE: u32 = 2;

#[repr(C)]
struct bcf_hdr_t { _unused: [u8; 0], }

#[link(name = "hts")]
extern "C" {
	fn bcf_hdr_init(mode: *const c_char) -> *mut bcf_hdr_t;
	fn bcf_hdr_destroy(hdr: *mut bcf_hdr_t);
	fn bcf_hdr_append(hdr: *mut bcf_hdr_t, line: *const c_char) -> c_int;
	fn bcf_hdr_get_version(hdr: *const bcf_hdr_t) -> *const c_char;
	fn bcf_hdr_add_sample(hdr: *mut bcf_hdr_t, sample: *const c_char) -> c_int;
	fn bcf_hdr_write(fp: *mut htsFile, hdr: *mut bcf_hdr_t) -> c_int;
	fn bcf_hdr_id2int(hdr: *const bcf_hdr_t, type_: c_int, id: *const c_char) -> c_int;
	fn bcf_hdr_sync(hdr: *mut bcf_hdr_t) -> c_int;
	fn bcf_init() -> *mut bcf1_t;
	fn bcf_destroy(bcf: *mut bcf1_t);
	fn bcf_clear(bcf: *mut bcf1_t);
	fn bcf_write(hfile: *mut htsFile, hdr: *mut bcf_hdr_t, brec: *mut bcf1_t) -> c_int;
}

pub struct VcfHeader {
	inner: NonNull<bcf_hdr_t>,
}

unsafe impl Sync for VcfHeader {}
unsafe impl Send for VcfHeader {}

impl VcfHeader {
	pub fn new(mode: &str) -> io::Result<VcfHeader> {
		match NonNull::new(unsafe{ bcf_hdr_init(get_cstr(mode).as_ptr())}) {
			None => Err(hts_err("Couldn't create VCF/BCF header".to_string())),
			Some(hdr) => Ok(VcfHeader{inner: hdr}),
		}
	}
	fn inner(&self) -> &bcf_hdr_t { unsafe{self.inner.as_ref()} }
	fn inner_mut(&mut self) -> &mut bcf_hdr_t { unsafe{ self.inner.as_mut() }} 
	
	pub fn append<S: AsRef<str>>(&mut self, line: S) -> io::Result<()> {
		match unsafe{ bcf_hdr_append(self.inner_mut(), get_cstr(line).as_ptr())} {
			0 => Ok(()),
			_ => Err(hts_err("Error appending line to VCF/BCF header".to_string()))
		}
	}
	
	pub fn get_version(&self) -> &str {
		from_cstr(unsafe { bcf_hdr_get_version(self.inner()) })
	}
	
	pub fn add_sample<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> {
		match unsafe{ bcf_hdr_add_sample(self.inner_mut(), get_cstr(name).as_ptr())} {
			0 => Ok(()),
			_ => Err(hts_err("Error adding sample to VCF/BCF header".to_string()))
		}
	}	
	pub fn write(&mut self, hout: &mut HtsFile) -> io::Result<()> {
		match unsafe { bcf_hdr_write(hout.inner_mut(), self.inner_mut()) } {
			0 => Ok(()),
			_ => Err(hts_err("Error writing VCF/BCF header".to_string()))			
		}
	}
	pub fn id2int<S: AsRef<str>>(&self, category: usize, name: S) -> Option<usize> {
		let ix = unsafe { bcf_hdr_id2int(self.inner(), category as libc::c_int, get_cstr(name.as_ref()).as_ptr()) };
		if ix < 0 { None } else { Some(ix as usize) }
	}
	pub fn sync(&mut self) -> io::Result<()> {
		match unsafe{ bcf_hdr_sync(self.inner_mut())} {
			0 => Ok(()),
			_ => Err(hts_err("Error adding sample to VCF/BCF header".to_string()))
		}		
	}
}

impl Drop for VcfHeader {
	fn drop(&mut self) {
		unsafe { bcf_hdr_destroy(self.inner_mut()) };
	}
}

pub const BCF_BT_NULL: u8 = 0;
pub const BCF_BT_INT8: u8 = 1;
pub const BCF_BT_INT16: u8 = 2;
pub const BCF_BT_INT32: u8 = 3;
pub const BCF_BT_INT64: u8 = 4;
pub const BCF_BT_FLOAT: u8 = 5;
pub const BCF_BT_CHAR: u8 = 7;

pub const BCF_MAX_BT_INT8: i32 = 0x7f;          /* INT8_MAX  */
pub const BCF_MAX_BT_INT16: i32 = 0x7fff;      /* INT16_MAX */
pub const MAX_BT_INT32: i32 = 0x7fffffff;      /* INT32_MAX */
pub const BCF_MIN_BT_INT8: i32 = -120;          /* INT8_MIN  + 8 */
pub const BCF_MIN_BT_INT16: i32 = -32760;      /* INT16_MIN + 8 */
pub const BCF_MIN_BT_INT32: i32 = -2147483640; /* INT32_MIN + 8 */

#[allow(non_upper_case_globals)]
pub const bcf_int8_vector_end: i32 = -127;                   /* INT8_MIN  + 1 */
#[allow(non_upper_case_globals)]
pub const bcf_int16_vector_end: i32 = -32767;               /* INT16_MIN + 1 */
#[allow(non_upper_case_globals)]
pub const bcf_int32_vector_end: i32 = -2147483647;          /* INT32_MIN + 1 */
#[allow(non_upper_case_globals)]
pub const bcf_int64_vector_end: i64 = -9223372036854775807; /* INT64_MIN + 1 */
#[allow(non_upper_case_globals)]
pub const bcf_str_vector_end: usize = 0;
#[allow(non_upper_case_globals)]
pub const bcf_int8_missing: i32 = -128;                      /* INT8_MIN  */
#[allow(non_upper_case_globals)]
pub const bcf_int16_missing: i32 = -32767-1;                /* INT16_MIN */
#[allow(non_upper_case_globals)]
pub const bcf_int32_missing: i32 = -2147483647-1;            /* INT32_MIN */
#[allow(non_upper_case_globals)]
pub const bcf_int64_missing: i64 = -9223372036854775807-1;  /* INT64_MIN */
#[allow(non_upper_case_globals)]
pub const bcf_str_missing: usize = 0x07;

#[repr(C)] 
#[derive(BitfieldStruct)]
struct bcf1_t {
	pos: HtsPos,
	rlen: HtsPos,
	rid: i32,
	qual: f32,
	#[bitfield(name = "n_info", ty = "u16", bits = "0..=15")]
	#[bitfield(name = "n_allele", ty = "u16", bits = "16..=31")]
	bitfield1: [u8; 4],
	#[bitfield(name = "n_fmt", ty = "u8", bits = "0..=7")]
	#[bitfield(name = "n_sample", ty = "u32", bits = "8..=31")]
	bitfield2: [u8; 4],
	shared: kstring_t,
	indiv: kstring_t,
	_unused: [u8; 0],
}

pub struct BcfRec {
	inner: NonNull<bcf1_t>,
}

unsafe impl Sync for BcfRec {}
unsafe impl Send for BcfRec {}

impl BcfRec {
	pub fn new() -> io::Result<Self> { 
		match NonNull::new(unsafe{bcf_init()}) {
			Some(b) => Ok(Self{inner: b}),
			None => Err(hts_err("Failed to allocate new BcfRec".to_string())),
		}
	}
//	fn inner(&self) -> &bcf1_t { unsafe {self.inner.as_ref() }}	
	fn inner_mut(&mut self) -> &mut bcf1_t { unsafe {self.inner.as_mut() }}	
	pub fn clear(&mut self) { unsafe{ bcf_clear(self.inner.as_ptr())} }
	pub fn shared(&mut self) -> &mut kstring_t { &mut self.inner_mut().shared }
	pub fn indiv(&mut self) -> &mut kstring_t { &mut self.inner_mut().indiv }
	pub fn set_rid(&mut self, rid: usize) { self.inner_mut().rid = rid as i32 }
	pub fn set_pos(&mut self, pos: usize) { self.inner_mut().pos = pos as HtsPos }
	pub fn set_rlen(&mut self, rlen: usize) { self.inner_mut().rlen = rlen as HtsPos }
	pub fn set_n_allele(&mut self, n_all: usize) { self.inner_mut().set_n_allele(n_all as u16) }
	pub fn set_n_info(&mut self, n_info: usize) { self.inner_mut().set_n_info(n_info as u16) }
	pub fn set_n_sample(&mut self, n_sample: usize) { self.inner_mut().set_n_sample(n_sample as u32) }
	pub fn set_n_fmt(&mut self, n_fmt: usize) { self.inner_mut().set_n_fmt(n_fmt as u8) }
	pub fn set_qual(&mut self, qual: f32) { self.inner_mut().qual = qual }
	pub fn write(&mut self, file: &mut HtsFile, hdr: &mut VcfHeader) -> io::Result<()> {
		if unsafe { bcf_write(file.inner_mut(), hdr.inner_mut(), self.inner_mut()) } < 0 { Err(hts_err("Error writing out VCF record".to_string())) } else { Ok(()) }
	}
}

impl Drop for BcfRec { 
	fn drop(&mut self) { unsafe{ bcf_destroy(self.inner.as_ptr())} }
}

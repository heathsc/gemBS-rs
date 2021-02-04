use std::io;
use std::ptr::NonNull;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use libc::{c_char, c_int, c_void};
use super::{hts_err, get_cstr, htsFile, htsThreadPool, tbx_t, hts_idx_t, hts_itr_t, HtsPos, kstring_t, bcf_hdr_t, bcf1_t};

#[repr(C)]
#[allow(non_camel_case_types)]
pub enum bcf_sr_error { 
	open_failed, not_bgzf, idx_load_failed, file_type_error, api_usage_error,
    header_error, no_eof, no_memory, vcf_parse_error, bcf_read_error, noidx_error
}

#[repr(C)]
struct bcf_sr_t {
	file: *mut htsFile,
	tbx_idx: *mut tbx_t,
	bcf_idx: *mut hts_idx_t,
	header: *mut bcf_hdr_t,
	itr: *mut hts_itr_t,
	fname: *mut c_char,
	buffer: *mut *mut bcf1_t,
	nbuffer: c_int,
	mbuffer: c_int,
	nfilter_ids: c_int,
	filter_ids: *mut c_int,
	samples: *mut c_int,
	n_smpl: c_int,
}

#[repr(C)]
struct region1_t {
	start: HtsPos,
	end: HtsPos,
}

#[repr(C)]
struct bcf_sr_region_t {
	regs: *mut region1_t,
	nregs: c_int,
	mregs: c_int,
	creg: c_int,
}

#[repr(C)]
struct bcf_sr_regions_t {
	tbx: *mut tbx_t,
	its: *mut hts_itr_t,
	line: kstring_t,
	file: *mut htsFile,
	fname: *mut c_char,
	is_bin: c_int,
	als: *mut *mut c_char,
	als_str: kstring_t,
	nals: c_int,
	mals: c_int,
	als_type: c_int,
	missed_ref_handler: unsafe extern "C" fn (reg: *mut bcf_sr_regions_t, p: *mut c_void),
	missed_reg_data: *mut c_void,
	regs: *mut bcf_sr_region_t,
	seq_hash: *mut c_void,
	seq_names: *mut *mut c_char,
	nseqs: c_int,
	iseq: c_int,
	start: HtsPos,
	end: HtsPos,
	prev_seq: c_int,
	prev_start: HtsPos,
	prev_end: HtsPos,  
}

#[repr(C)]
pub struct bcf_srs_t {
	collapse: c_int,
	apply_filters: *mut c_char,
	require_index: c_int,
	max_unpack: c_int,
	has_line: *mut c_int,
	errnum: bcf_sr_error,
	readers: *mut bcf_sr_t,
	nreaders: c_int,
	streaming: c_int,
	explicit_regs: c_int,
	samples: *mut *mut c_char,
	regions: *mut bcf_sr_regions_t,
	targets: *mut bcf_sr_regions_t,
	targets_als: c_int,
	targets_exclude: c_int,
	tmps: kstring_t,
	n_smpl: c_int,
	n_threads: c_int,
	p: *mut htsThreadPool,
	aux: *mut c_void,	
}

impl bcf_srs_t {
	pub fn set_regions<S: AsRef<str>>(&mut self, regions: S, is_file: bool) -> io::Result<()> {
		match unsafe{bcf_sr_set_regions(self, get_cstr(regions.as_ref()).as_ptr(), if is_file {1} else {0})} {
			0 => Ok(()),
			_ => Err(hts_err("Couldn't set BCF regions".to_string())),
		}
	}
	pub fn set_threads(&mut self, n_threads: usize) -> io::Result<()> {
		match unsafe{bcf_sr_set_threads(self, n_threads as c_int)} {
			0 => Ok(()),
			_ => Err(hts_err("Couldn't set BCF threads".to_string())),			
		}
	}
	pub fn add_reader<S: AsRef<str>>(&mut self, fname: S) -> io::Result<()> {
		match unsafe{bcf_sr_add_reader(self, get_cstr(fname.as_ref()).as_ptr())} {
			1 => Ok(()),
			_ => Err(hts_err(format!("Couldn't open BCF file {}", fname.as_ref()))),
		}
	}
	fn get_reader(&mut self, idx: usize) -> io::Result<*mut bcf_sr_t> {
		unsafe {
			if (idx as c_int) >= self.nreaders || self.readers.is_null() { Err(hts_err("Invalid access to synced BCF reader".to_string()))}
			else { 
				let rdr = self.readers.add(idx);
				if rdr.is_null() { Err(hts_err("Synced reader is null".to_string()))}
				else { Ok(rdr) }
			}
		}		
	}
	pub fn get_reader_hdr(&mut self, idx: usize) -> io::Result<&bcf_hdr_t> {
		unsafe {
			let rdr = self.get_reader(idx)?;
			let hdr = rdr.as_ref().unwrap().header;
			if hdr.is_null() { Err(hts_err("Null BCF header in synced BCF reader".to_string())) }
			else { Ok(hdr.as_ref().unwrap()) }
		}
	}
	pub fn next_line(&mut self) -> c_int { unsafe {bcf_sr_next_line(self)}}

	pub fn swap_line<B: AsMut<bcf1_t>>(&mut self, idx: usize, mut brec: B) -> io::Result<()> {
		let rdr = self.get_reader(idx)?;
		unsafe { std::ptr::swap(brec.as_mut(), *rdr.as_ref().unwrap().buffer) }
		Ok(())
	} 	
}

#[link(name = "hts")]
extern "C" {
	fn bcf_sr_init() -> *mut bcf_srs_t;
	fn bcf_sr_destroy(readers: *mut bcf_srs_t);
	fn bcf_sr_set_regions(readers: *mut bcf_srs_t, regions: *const c_char, is_file: c_int) -> c_int;
	fn bcf_sr_set_threads(readers: *mut bcf_srs_t, n_threads: c_int) -> c_int;
	fn bcf_sr_add_reader(readers: *mut bcf_srs_t, fname: *const c_char) -> c_int;
	fn bcf_sr_next_line(readers: *mut bcf_srs_t) -> c_int;
}

pub struct BcfSrs {
	inner: NonNull<bcf_srs_t>,	
	phantom: PhantomData<bcf_srs_t>,
}

impl Deref for BcfSrs {
	type Target = bcf_srs_t;
	#[inline]
	fn deref(&self) -> &bcf_srs_t { unsafe{self.inner.as_ref()} }	
}

impl DerefMut for BcfSrs {
	#[inline]
	fn deref_mut(&mut self) -> &mut bcf_srs_t {unsafe{ self.inner.as_mut() }}
}

impl AsRef<bcf_srs_t> for BcfSrs {
	#[inline]
	fn as_ref(&self) -> &bcf_srs_t { self}	
}

impl AsMut<bcf_srs_t> for BcfSrs {
	#[inline]
	fn as_mut(&mut self) -> &mut bcf_srs_t { self}	
}

impl BcfSrs {
	pub fn new() -> io::Result<Self> { 
		match NonNull::new(unsafe{ bcf_sr_init()}) {
			None => Err(hts_err("Couldn't create BCF synced reader".to_string())),
			Some(sr) => Ok(Self{inner: sr, phantom: PhantomData}),
		}		
	}
}

impl Drop for BcfSrs {
	fn drop(&mut self) { unsafe {bcf_sr_destroy(self.as_mut())};}
}

use std::io;
use std::ptr::{null_mut, NonNull};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use libc::{c_char, c_int, c_void};
use super::{hts_err, get_cstr, from_cstr, htsFile, htsThreadPool, tbx_t, hts_idx_t, hts_itr_t, HtsPos, kstring_t, bcf_hdr_t, bcf1_t};

#[repr(C)]
#[allow(non_camel_case_types)]
pub enum bcf_sr_error { 
	open_failed, not_bgzf, idx_load_failed, file_type_error, api_usage_error,
    header_error, no_eof, no_memory, vcf_parse_error, bcf_read_error, noidx_error
}

#[repr(C)]
pub struct bcf_sr_t {
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

impl bcf_sr_t {
	pub fn file(&mut self) -> &mut htsFile { unsafe {self.file.as_mut()}.unwrap() }
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

impl bcf_sr_region_t {
	fn get_reg(&self, ix: usize) -> io::Result<&region1_t> {
		if (ix as c_int) >= self.nregs || self.regs.is_null() { Err(hts_err("Invalid access to BCF region".to_string()))}
		else { Ok(unsafe{self.regs.add(ix).as_ref().expect("Invalid region")}) }	
	}	
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

impl bcf_sr_regions_t {
	fn seq_name(&self, ix: usize) -> io::Result<&str> {
		if (ix as c_int) >= self.nseqs || self.seq_names.is_null() { Err(hts_err("Invalid access to synced BCF region".to_string()))}
		else { Ok(from_cstr(unsafe{*(self.seq_names.add(ix))})) }
	}	
	fn seq_regs(&self, ix: usize) -> io::Result<&bcf_sr_region_t> {
		if (ix as c_int) >= self.nseqs || self.regs.is_null() { Err(hts_err("Invalid access to synced BCF region".to_string()))}
		else { Ok(unsafe{self.regs.add(ix).as_ref().expect("Invalid region")}) }		
	}
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
	pub fn get_reader(&mut self, idx: usize) -> io::Result<&mut bcf_sr_t> {
		unsafe {
			if (idx as c_int) >= self.nreaders || self.readers.is_null() { Err(hts_err("Invalid access to synced BCF reader".to_string()))}
			else { 
				self.readers.add(idx).as_mut().ok_or_else(|| hts_err("Synced reader is null".to_string()))
			}
		}		
	}
	pub fn get_reader_hdr(&mut self, idx: usize) -> io::Result<&bcf_hdr_t> {
		unsafe {
			let rdr = self.get_reader(idx)?;
			let hdr = rdr.header;
			if hdr.is_null() { Err(hts_err("Null BCF header in synced BCF reader".to_string())) }
			else { Ok(hdr.as_ref().unwrap()) }
		}
	}
	pub fn next_line(&mut self) -> c_int { unsafe {bcf_sr_next_line(self)}}

	pub fn swap_line<B: AsMut<bcf1_t>>(&mut self, idx: usize, mut brec: B) -> io::Result<()> {
		let rdr = self.get_reader(idx)?;
		unsafe { std::ptr::swap(brec.as_mut(), *rdr.buffer) }
		Ok(())
	} 
	pub fn thread_pool(&self) -> Option<&htsThreadPool> { unsafe { self.p.as_ref() }}
	
	// Sort regions (if they exist) by chromosome name
	pub fn sort_regions(&mut self) {
		if let Some(mut p) = NonNull::new(self.regions) {
			let reg = unsafe {p.as_ref()};
			let nseq = reg.nseqs;
			let mut ix: Vec<usize> = (0..nseq).map(|x| x as usize).collect();
			ix.sort_unstable_by_key(|i| reg.seq_name(*i).unwrap());
			let mut reg_string = String::new();
			let mut first = true;
			for i in ix.iter() {
				let seq = reg.seq_name(*i).unwrap();
				let rgs = reg.seq_regs(*i).unwrap();
				for j in 0..rgs.nregs {
					let rg = rgs.get_reg(j as usize).unwrap();
					if rg.start <= rg.end { 
						if !first { reg_string.push(',') }
						else { first = false }
						reg_string.push_str(format!("{}:{}-{}", seq, rg.start, rg.end).as_ref()); 
					}
				}
			}
			if reg_string.is_empty() { panic!("No valid regions found!")}
			unsafe {bcf_sr_regions_destroy(p.as_mut())};
			self.regions = null_mut();
			self.set_regions(&reg_string, false).unwrap();
		}
	}
}

#[link(name = "hts")]
extern "C" {
	fn bcf_sr_init() -> *mut bcf_srs_t;
	fn bcf_sr_destroy(readers: *mut bcf_srs_t);
	fn bcf_sr_set_regions(readers: *mut bcf_srs_t, regions: *const c_char, is_file: c_int) -> c_int;
	fn bcf_sr_regions_destroy(regions: *mut bcf_sr_regions_t);
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

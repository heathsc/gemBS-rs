use std::io;
use std::ptr::NonNull;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use c2rust_bitfields::BitfieldStruct;
use libc::{c_char, c_int, c_void};
use super::{hts_err, get_cstr, from_cstr, htsFile, htsThreadPool, tbx_t, hts_idx_t, hts_itr_t, HtsFile, HtsPos, kstring_t, MallocDataBlock};

pub const BCF_DT_ID: u32 = 0;
pub const BCF_DT_CTG: u32 = 1;
pub const BCF_DT_SAMPLE: u32 = 2;

#[repr(C)]
struct bcf_hrec_t { 
	_type: c_int,
	key: *mut c_char,
	val: *mut c_char,
	nkeys: c_int,
	keys: *mut *mut c_char,
	vals: *mut *mut c_char,
}

#[repr(C)]
struct bcf_idinfo_t { 
	info: [u64; 3],
	hrec: [*mut bcf_hrec_t; 3],
	id: c_int,
}

#[repr(C)]
struct bcf_idpair_t { 
	key: *const c_char,
	val: *const bcf_idinfo_t,	
}

#[repr(C)] 
struct vdict_t {
	n_buckets: u32,
	size: u32,
	n_occupied: u32,
	upper_bound: u32,
	flags: *mut u32,
	keys: *mut c_void,
	vals: *mut c_void, 	
}

#[repr(C)]
pub struct bcf_hdr_t { 
	n: [i32; 3],
	id: [*mut bcf_idpair_t; 3],
	dict: [*mut vdict_t; 3],
	samples: *mut *mut c_char,
	hrec: *mut *mut bcf_hrec_t,
	nhrec: c_int,
	dirty: c_int,
	ntransl: c_int,
	transl: [*mut c_int; 2],
	n_samples_ori: c_int,
	keep_samples: *mut u8,
	mem: kstring_t,
	m: [i32; 3],
}

impl bcf_hdr_t {
	pub fn nsamples(&self) -> usize {self.n[BCF_DT_SAMPLE as usize] as usize}
	pub fn dup(&self) -> VcfHeader {
		match NonNull::new(unsafe{ bcf_hdr_dup(self) })	{
			None => panic!("Couldn't duplicate VCF/BCF header"),
			Some(hdr) => VcfHeader{inner: hdr, phantom: PhantomData},
		}	
	}
	pub fn get_version(&self) -> &str {
		from_cstr(unsafe { bcf_hdr_get_version(self) })
	}
	pub fn append<S: AsRef<str>>(&mut self, line: S) -> io::Result<()> {
		match unsafe{ bcf_hdr_append(self, get_cstr(line).as_ptr())} {
			0 => Ok(()),
			_ => Err(hts_err("Error appending line to VCF/BCF header".to_string()))
		}
	}
	
	pub fn add_sample<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> {
		match unsafe{ bcf_hdr_add_sample(self, get_cstr(name).as_ptr())} {
			0 => Ok(()),
			_ => Err(hts_err("Error adding sample to VCF/BCF header".to_string()))
		}
	}	
	pub fn nctgs(&self) -> usize {self.n[BCF_DT_CTG as usize] as usize}
	pub fn ctg_name(&self, rid: usize) -> io::Result<&str> {
		if rid >= self.nctgs() { Err(hts_err("Invalid contig id".to_string()))}
		else {
			let p = unsafe {self.id[BCF_DT_CTG as usize].add(rid).as_ref()}.ok_or_else(|| hts_err("Invalid contig id".to_string()))?.key;
			Ok(from_cstr(p))
		}
	}
	pub fn copy_ctgs(&self) -> Box<[Box<str>]> {
		let nctgs = self.nctgs();
		let mut ctg_names = Vec::with_capacity(nctgs);
		for rid in 0..nctgs {
			let s = from_cstr(unsafe {self.id[BCF_DT_CTG as usize].add(rid).as_ref()}.expect("Invalid contig id").key).to_owned();
			ctg_names.push(s.into_boxed_str());
		}
		ctg_names.into_boxed_slice()
	}
	pub fn write(&mut self, hout: &mut HtsFile) -> io::Result<()> {
		match unsafe { bcf_hdr_write(hout.inner_mut(), self) } {
			0 => Ok(()),
			_ => Err(hts_err("Error writing VCF/BCF header".to_string()))			
		}
	}
	pub fn id2int<S: AsRef<str>>(&self, category: usize, name: S) -> Option<usize> {
		let ix = unsafe { bcf_hdr_id2int(self, category as libc::c_int, get_cstr(name.as_ref()).as_ptr()) };
		if ix < 0 { None } else { Some(ix as usize) }
	}
	pub fn sync(&mut self) -> io::Result<()> {
		match unsafe{ bcf_hdr_sync(self)} {
			0 => Ok(()),
			_ => Err(hts_err("Error adding sample to VCF/BCF header".to_string()))
		}		
	}
	
}

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
struct bcf_srs_t {
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


#[link(name = "hts")]
extern "C" {
	fn bcf_hdr_init(mode: *const c_char) -> *mut bcf_hdr_t;
	fn bcf_hdr_dup(hdr: *const bcf_hdr_t) -> *mut bcf_hdr_t;
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
	fn bcf_sr_init() -> *mut bcf_srs_t;
	fn bcf_sr_destroy(readers: *mut bcf_srs_t);
	fn bcf_sr_set_regions(readers: *mut bcf_srs_t, regions: *const c_char, is_file: c_int) -> c_int;
	fn bcf_sr_set_threads(readers: *mut bcf_srs_t, n_threads: c_int) -> c_int;
	fn bcf_sr_add_reader(readers: *mut bcf_srs_t, fname: *const c_char) -> c_int;
	fn bcf_sr_next_line(readers: *mut bcf_srs_t) -> c_int;
	fn bcf_unpack(b: *mut bcf1_t, which: c_int);
	fn bcf_get_format_values(hdr: *const bcf_hdr_t, line: *mut bcf1_t, tag: *const c_char, dst: *mut *mut c_void, ndst: *mut c_int, _type: c_int) -> c_int;
	fn bcf_get_info_values(hdr: *const bcf_hdr_t, line: *mut bcf1_t, tag: *const c_char, dst: *mut *mut c_void, ndst: *mut c_int, _type: c_int) -> c_int;
}

pub struct BcfSrs {
	inner: NonNull<bcf_srs_t>,	
}

impl BcfSrs {
	pub fn new() -> io::Result<Self> { 
		match NonNull::new(unsafe{ bcf_sr_init()}) {
			None => Err(hts_err("Couldn't create BCF synced reader".to_string())),
			Some(sr) => Ok(Self{inner: sr}),
		}		
	}
//	fn inner(&self) -> &bcf_srs_t { unsafe{self.inner.as_ref()} }
	fn inner_mut(&mut self) -> &mut bcf_srs_t { unsafe{ self.inner.as_mut() }} 	
	pub fn set_regions<S: AsRef<str>>(&mut self, regions: S, is_file: bool) -> io::Result<()> {
		match unsafe{bcf_sr_set_regions(self.inner_mut(), get_cstr(regions.as_ref()).as_ptr(), if is_file {1} else {0})} {
			0 => Ok(()),
			_ => Err(hts_err("Couldn't set BCF regions".to_string())),
		}
	}
	pub fn set_threads(&mut self, n_threads: usize) -> io::Result<()> {
		match unsafe{bcf_sr_set_threads(self.inner_mut(), n_threads as c_int)} {
			0 => Ok(()),
			_ => Err(hts_err("Couldn't set BCF threads".to_string())),			
		}
	}
	pub fn add_reader<S: AsRef<str>>(&mut self, fname: S) -> io::Result<()> {
		match unsafe{bcf_sr_add_reader(self.inner_mut(), get_cstr(fname.as_ref()).as_ptr())} {
			1 => Ok(()),
			_ => Err(hts_err(format!("Couldn't open BCF file {}", fname.as_ref()))),
		}
	}
	fn get_reader(&mut self, idx: usize) -> io::Result<*mut bcf_sr_t> {
		unsafe {
			let sr = self.inner_mut();
			if (idx as c_int) >= sr.nreaders || sr.readers.is_null() { Err(hts_err("Invalid access to synced BCF reader".to_string()))}
			else { 
				let rdr = sr.readers.add(idx);
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
	pub fn next_line(&mut self) -> c_int { unsafe {bcf_sr_next_line(self.inner_mut())}}

	pub fn swap_line(&mut self, idx: usize, brec: &mut BcfRec) -> io::Result<()> {
		let rdr = self.get_reader(idx)?;
		unsafe { std::ptr::swap(brec.inner.as_ptr(), *rdr.as_ref().unwrap().buffer) }
		Ok(())
	} 
}

impl Drop for BcfSrs {
	fn drop(&mut self) { unsafe {bcf_sr_destroy(self.inner_mut())};}
}

pub struct VcfHeader {
	inner: NonNull<bcf_hdr_t>,
	phantom: PhantomData<bcf_hdr_t>,
}

impl Deref for VcfHeader {
	type Target = bcf_hdr_t;
	#[inline]
	fn deref(&self) -> &bcf_hdr_t { unsafe{self.inner.as_ref()} }	
}

impl DerefMut for VcfHeader {
	#[inline]
	fn deref_mut(&mut self) -> &mut bcf_hdr_t {unsafe{ self.inner.as_mut() }}
}

impl AsRef<bcf_hdr_t> for VcfHeader {
	#[inline]
	fn as_ref(&self) -> &bcf_hdr_t { self}	
}

impl AsMut<bcf_hdr_t> for VcfHeader {
	#[inline]
	fn as_mut(&mut self) -> &mut bcf_hdr_t { self}	
}

unsafe impl Sync for VcfHeader {}
unsafe impl Send for VcfHeader {}

impl VcfHeader {
	pub fn new(mode: &str) -> io::Result<VcfHeader> {
		match NonNull::new(unsafe{ bcf_hdr_init(get_cstr(mode).as_ptr())}) {
			None => Err(hts_err("Couldn't create VCF/BCF header".to_string())),
			Some(hdr) => Ok(VcfHeader{inner: hdr, phantom: PhantomData}),
		}
	}
	pub fn as_bcf_hdr_t(&self) -> &bcf_hdr_t { self}
	pub fn as_mut_bcf_hdr_t(&mut self) -> &mut bcf_hdr_t { self}
}



impl Drop for VcfHeader {
	fn drop(&mut self) {
		unsafe { bcf_hdr_destroy(self.as_mut_bcf_hdr_t())};
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

pub const BCF_UN_STR: usize = 1;    // up to ALT inclusive
pub const BCF_UN_FLT: usize = 2;    // up to FILTER
pub const BCF_UN_INFO: usize = 4;   // up to INFO
pub const BCF_UN_SHR: usize = BCF_UN_STR|BCF_UN_FLT|BCF_UN_INFO;    // All sgared information
pub const BCF_UN_FMT: usize = 8;           // unpack format and each sample  
pub const BCF_UN_IND: usize = BCF_UN_FMT;  // a synonym of BCF_UN_FMT
pub const BCF_UN_ALL: usize = BCF_UN_SHR|BCF_UN_FMT; // everything

pub const BCF_HT_INT: c_int = 1;
pub const BCF_HT_REAL: c_int = 2;
pub const BCF_HT_STR: c_int = 3;

#[repr(C)] 
struct bcf_info_t {
    _unused: [u8; 0],	
}
#[repr(C)] 
#[derive(BitfieldStruct)]
struct bcf_fmt_t {
    id: c_int,
	n: c_int,
	size: c_int,
	_type: c_int,
	p: *mut u8,
	p_len: u32,
	#[bitfield(name = "p_off", ty = "u32", bits = "0..=30")]
	#[bitfield(name = "p_free", ty = "u8", bits = "31..=31")]
	bitfield1: [u8; 4],
}
#[repr(C)] 
struct bcf_variant_t {
    _unused: [u8; 0],	
}

#[repr(C)] 
struct bcf_dec_t {
	m_fmt: c_int,
	m_info: c_int,
	m_id: c_int,
	m_als: c_int,
	m_allele: c_int,
	m_flt: c_int,
	n_flt: c_int,
	flt: *mut c_int,
	id: *mut c_char,
	als: *mut c_char,
	alleles: *mut *mut c_char,
	info: *mut bcf_info_t,
	fmt: *mut bcf_fmt_t,
	var: *mut bcf_variant_t,
	n_var: c_int,
	var_type: c_int,
	shared_dirty: c_int,
	indiv_dirty: c_int,	
}

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
	d: bcf_dec_t,
	max_unpack: c_int,
	unpacked: c_int,
	unpack_size: [c_int; 3],
	errcode: c_int,
}

pub struct BcfRec {
	inner: NonNull<bcf1_t>,
}

unsafe impl Sync for BcfRec {}
unsafe impl Send for BcfRec {}

fn prepare_format_args<T>(tag: &str, buf: &mut MallocDataBlock<T>) -> (CString, *mut T, c_int) {
	let (p, _, cap) = unsafe {buf.raw_parts()};
	let cap = cap as c_int;
	let tag = CString::new(tag).unwrap();
	(tag, p, cap)		
}

fn ret_format_res<T>(p: *mut T, len: c_int, cap: c_int, buf: &mut MallocDataBlock<T>) -> Option<usize> {
	unsafe{buf.update_raw_parts(p, len as usize, cap as usize)};
	if len < 0 { None }
	else { Some(buf.len()) }
}

impl BcfRec {
	pub fn new() -> io::Result<Self> { 
		match NonNull::new(unsafe{bcf_init()}) {
			Some(b) => Ok(Self{inner: b}),
			None => Err(hts_err("Failed to allocate new BcfRec".to_string())),
		}
	}
	fn inner(&self) -> &bcf1_t { unsafe {self.inner.as_ref() }}	
	fn inner_mut(&mut self) -> &mut bcf1_t { unsafe {self.inner.as_mut() }}	
	pub fn clear(&mut self) { unsafe{ bcf_clear(self.inner.as_ptr())} }
	pub fn shared(&mut self) -> &mut kstring_t { &mut self.inner_mut().shared }
	pub fn indiv(&mut self) -> &mut kstring_t { &mut self.inner_mut().indiv }
	pub fn set_rid(&mut self, rid: usize) { self.inner_mut().rid = rid as i32 }
	pub fn rid(&self) -> usize { self.inner().rid as usize}
	pub fn set_pos(&mut self, pos: usize) { self.inner_mut().pos = pos as HtsPos }
	pub fn pos(&self) -> usize { self.inner().pos as usize }
	pub fn set_rlen(&mut self, rlen: usize) { self.inner_mut().rlen = rlen as HtsPos }
	pub fn set_n_allele(&mut self, n_all: usize) { self.inner_mut().set_n_allele(n_all as u16) }
	pub fn set_n_info(&mut self, n_info: usize) { self.inner_mut().set_n_info(n_info as u16) }
	pub fn set_n_sample(&mut self, n_sample: usize) { self.inner_mut().set_n_sample(n_sample as u32) }
	pub fn set_n_fmt(&mut self, n_fmt: usize) { self.inner_mut().set_n_fmt(n_fmt as u8) }
	pub fn set_qual(&mut self, qual: f32) { self.inner_mut().qual = qual }
	pub fn write<H: AsMut<bcf_hdr_t>>(&mut self, file: &mut HtsFile, mut hdr: H) -> io::Result<()> {
		if unsafe { bcf_write(file.inner_mut(), hdr.as_mut(), self.inner_mut()) } < 0 { Err(hts_err("Error writing out VCF record".to_string())) } else { Ok(()) }
	}
	pub fn unpack(&mut self, which: usize) { unsafe{bcf_unpack(self.inner_mut(), which as c_int)} }
	pub fn id(&mut self) -> &str {
		self.unpack(BCF_UN_STR);
		from_cstr(self.inner_mut().d.id)	
	}
	pub fn check_pass(&mut self) -> bool {
		self.unpack(BCF_UN_FLT);
		let d = &self.inner().d;
		let flt = d.flt;
		if flt.is_null() { panic!("BCF record filter is null") }
		for i in 0..(d.n_flt as usize) { if unsafe{*d.flt.add(i)} == 0 { return true }}
		false
	}
	pub fn alleles(&mut self) -> Vec<&str> {
		self.unpack(BCF_UN_STR);
		let n_all = self.inner().n_allele() as usize;
		let mut v = Vec::with_capacity(n_all);
		let all = &self.inner().d.alleles;
		if all.is_null() { panic!("BCF allele desc is null")}
		for i in 0..n_all {	v.push(from_cstr(unsafe{*all.add(i)}))}		
		v		
	}
	pub fn get_format_values<T, H: AsRef<bcf_hdr_t>>(&mut self, hdr: H, tag: &str, buf: &mut MallocDataBlock<T>, vtype: c_int) -> Option<usize> {
		let (tag, mut p, mut cap) = prepare_format_args(tag, buf);
		let len = unsafe {bcf_get_format_values(hdr.as_ref(), self.inner_mut(), tag.as_ptr(), &mut p as *mut *mut T as *mut *mut c_void, &mut cap as *mut c_int, vtype)};
		ret_format_res(p, len, cap, buf)
	}
	pub fn get_format_i32(&mut self, hdr: &VcfHeader, tag: &str, buf: &mut MallocDataBlock<i32>) -> Option<usize> { self.get_format_values(hdr, tag, buf, BCF_HT_INT)}
	pub fn get_format_f32(&mut self, hdr: &VcfHeader, tag: &str, buf: &mut MallocDataBlock<f32>) -> Option<usize> { self.get_format_values(hdr, tag, buf, BCF_HT_REAL)}
	pub fn get_format_u8(&mut self, hdr: &VcfHeader, tag: &str, buf: &mut MallocDataBlock<u8>) -> Option<usize> { self.get_format_values(hdr, tag, buf, BCF_HT_STR)}
	pub fn get_genotypes(&mut self, hdr: &VcfHeader, buf: &mut MallocDataBlock<i32>) -> Option<usize> { self.get_format_i32(hdr, "GT", buf) }
	pub fn get_info_values<T, H: AsRef<bcf_hdr_t>>(&mut self, hdr: H, tag: &str, buf: &mut MallocDataBlock<T>, vtype: c_int) -> Option<usize> {
		let (tag, mut p, mut cap) = prepare_format_args(tag, buf);
		let len = unsafe {bcf_get_info_values(hdr.as_ref(), self.inner_mut(), tag.as_ptr(), &mut p as *mut *mut T as *mut *mut c_void, &mut cap as *mut c_int, vtype)};
		ret_format_res(p, len, cap, buf)
	}
	pub fn get_info_u8(&mut self, hdr: &VcfHeader, tag: &str, buf: &mut MallocDataBlock<u8>) -> Option<usize> { self.get_info_values(hdr, tag, buf, BCF_HT_STR)}
}

impl Drop for BcfRec { 
	fn drop(&mut self) { unsafe{ bcf_destroy(self.inner.as_ptr())} }
}


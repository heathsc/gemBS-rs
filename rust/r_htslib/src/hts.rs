use std::io;
use std::ptr::{null_mut, NonNull};
use std::ffi::{CString, c_void};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use libc::{c_char, c_int, c_short, c_uint, c_uchar, size_t, ssize_t, off_t};
use c2rust_bitfields::BitfieldStruct;
use super::{hts_err, get_cstr, SamReadResult, sam_hdr_t, bam1_t, kstring_t, tbx_t, TbxReadResult, BGZF};

pub type HtsPos = i64;

pub const HTS_POS_MAX: HtsPos = ((i32::MAX as i64) << 32) | (i32::MAX as i64);
pub const HTS_POS_MIN: HtsPos = i64::MIN;
pub const HTS_IDX_NOCOOR: HtsPos = -2;
pub const HTS_IDX_START: HtsPos = -3;
pub const HTS_IDX_REST: HtsPos = -4;
pub const HTS_IDX_NONE: HtsPos = -5;

pub const HTS_FMT_CSI: c_int = 0;
pub const HTS_FMT_BAI: c_int = 1;
pub const HTS_FMT_TBI: c_int = 2;
pub const HTS_FMT_CRAI: c_int = 3;
pub const HTS_FMT_FAI: c_int = 4;

pub const FT_UNKN: u32 = 0;
pub const FT_GZ: u32 = 1;
pub const FT_VCF: u32 = 2;
pub const FT_VCF_GZ: u32 = FT_GZ | FT_VCF;
pub const FT_BCF: u32 = 4;
pub const FT_BCF_GZ: u32 = FT_GZ | FT_BCF;
pub const FT_STDIN: u32 = 8;

#[repr(C)]
#[derive(BitfieldStruct)]
pub struct hFILE { 
	pub(crate) buffer: *mut c_char,
	pub(crate) begin: *mut c_char,
	pub(crate) end: *mut c_char,
	limit: *mut c_char,
	backend: *const c_void,
	pub(crate) offset: off_t,
	#[bitfield(name = "at_eof", ty = "c_uchar", bits = "0..=0")]
	#[bitfield(name = "mobile", ty = "c_uchar", bits = "1..=1")]
	#[bitfield(name = "readonly", ty = "c_uchar", bits = "2..=2")]
	bfield: [u8; 1],
	has_errno: c_int,
}

#[repr(C)]
pub struct cram_fd { _unused: [u8; 0], }

#[repr(C)]
union file_ptr {
	bgzf: *mut BGZF,
	cram: *mut cram_fd,
	hfile: *mut hFILE,
}

#[repr(C)]
#[derive(BitfieldStruct)]
pub struct htsFile { 
	#[bitfield(name = "is_bin", ty = "c_uchar", bits = "0..=0")]
	#[bitfield(name = "is_write", ty = "c_uchar", bits = "1..=1")]
	#[bitfield(name = "is_be", ty = "c_uchar", bits = "2..=2")]
	#[bitfield(name = "is_cram", ty = "c_uchar", bits = "3..=3")]
	#[bitfield(name = "is_bgzf", ty = "c_uchar", bits = "4..=4")]
	#[bitfield(name = "dummy", ty = "u32", bits = "5..=31")]
	bfield: [u8; 4],
	lineno: i64,
	line: kstring_t,
	fn_: *mut c_char,
	fn_aux: *mut c_char,
	fp: file_ptr,
	state: *mut c_void,
	format: htsFormat,
	idx: *mut hts_idx_t,
	fnidx: *const c_char,
	bam_header: *mut sam_hdr_t,
}

impl htsFile {
	pub fn set_threads(&mut self, t: usize) -> io::Result<()> {
		let ret = unsafe{hts_set_threads(self, t as c_int)};
		if ret != 0 { Err(hts_err("Failed to set additional threads to file".to_string())) } 
		else { Ok(()) }
	}
	pub fn format(&self) -> &htsFormat {	
		unsafe{hts_get_format(self).as_ref()}.expect("hts_get_format returned NULL")
	}
	pub fn set_fai_filename<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> {
		let name = name.as_ref();
		let ret = unsafe { hts_set_fai_filename(self, get_cstr(name).as_ptr()) };
		if ret != 0 { Err(hts_err(format!("Failed to attach reference index {} to file", name))) } 
		else { Ok(()) }
	}
	pub fn test_bgzf(&self) -> bool { 
		let c = self.is_bgzf();
		c != 0 
	}
	pub fn bgzf(&mut self) -> Option<&mut BGZF> {
		if self.test_bgzf() { unsafe {self.fp.bgzf.as_mut()} } else { None }
	}	
	pub fn set_thread_pool(&mut self, tp: &htsThreadPool) -> c_int {
		unsafe {hts_set_opt(self, hts_fmt_option::HtsOptThreadPool, tp)}
	}
}

impl io::Write for htsFile {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		let fp = &self.fp;
		let l = buf.len();
		let wlen = if self.format.compression != htsCompression::NoCompression { 
			let bgzf = unsafe { fp.bgzf };
			unsafe {bgzf_write(bgzf, buf.as_ptr() as *const c_void, l as libc::size_t)}
		} else {
			let hfile = unsafe { fp.hfile.as_mut().unwrap() };
			hwrite(hfile, buf.as_ptr() as *const c_void, l as libc::size_t)
		};
		if wlen < l as libc::ssize_t { Err(hts_err("htsFile::write Error writing to file".to_string())) }
		else { Ok(l) }
	}
	fn flush(&mut self) -> io::Result<()> { 
		let fp = &self.fp;
		let ret = if self.format.compression != htsCompression::NoCompression { 
			let bgzf = unsafe { fp.bgzf };
			unsafe { bgzf_flush(bgzf) }
		} else {
			let hfile = unsafe { fp.hfile.as_mut().unwrap() };
			unsafe{ hflush(hfile) }
		};
		if ret < 1  { Err(hts_err("htsFile::flush Error flushing buffers".to_string())) }
		else { Ok(()) }
	}
}

#[repr(C)]
pub struct hts_idx_t { _unused: [u8; 0], }

impl hts_idx_t {
	pub fn sam_itr_queryi(&self, tid: isize, start: usize, end: usize) -> io::Result<HtsItr> {
		HtsItr::new(unsafe{ sam_itr_queryi(self, tid as libc::c_int, start as HtsPos, end as HtsPos)}).ok_or_else(|| hts_err("Failed to obtain sam iterator".to_string()))
	}
	pub fn sam_itr_regarray<H: AsMut<sam_hdr_t>>(&self, mut hdr: H, regions: &[String]) -> io::Result<HtsItr> {
		let count = regions.len();
		if count == 0 { return self.sam_itr_queryi(HTS_IDX_START as isize, 0, 0) }
		// We need to do this in 2 stages: generate and array of CStrings and then an array of ptrs to the CStrings
		// otherwise the CStrings go out of scope before they are used and the ptrs point to garbage
		let carray: Vec<CString> = regions.iter().map(get_cstr).collect();
		let parray: Vec<*const c_char> = carray.iter().map(|cs| cs.as_ptr()).collect();
		HtsItr::new(unsafe{ sam_itr_regarray(self, hdr.as_mut(), parray.as_ptr(), count as c_uint)}).ok_or_else(|| hts_err("Failed to obtain sam iterator".to_string()))
	}
	pub fn push(&mut self, tid: usize, beg: usize, end: usize, offset: u64, is_mapped: bool) -> io::Result<()> {
		if unsafe{hts_idx_push(
			self, tid as c_int, beg as HtsPos, end as HtsPos, offset, if is_mapped { 1 } else { 0 }
		)} == 0 { Ok(()) }
		else { Err(hts_err("Error returned from hts_idx_push".to_string()))} 
	}
	pub fn finish(&mut self, final_offset: u64) -> io::Result<()> {
		if unsafe{hts_idx_finish(self, final_offset)} == 0 { Ok(()) } 
		else { Err(hts_err("Error returned from hts_idx_finish".to_string()))} 
	}
	pub fn set_meta(&mut self, meta: &[u8]) -> io::Result<()> {
		if unsafe{hts_idx_set_meta(self, meta.len() as u32, meta.as_ptr(), 1)} == 0 { Ok(()) }
		else { Err(hts_err("Error returned from hts_idx_set_meta".to_string()))} 
	}
	pub fn save_as<S: AsRef<str>>(&mut self, fname: S, fmt: c_int) -> io::Result<()> {
		if unsafe{hts_idx_save_as(self, get_cstr(fname.as_ref()).as_ptr(), std::ptr::null() as *const c_char, fmt)} == 0 { Ok(()) }
		else { Err(hts_err("Error returned from hts_idx_save_as".to_string()))}
	}
}

#[repr(C)]
#[derive(BitfieldStruct)]
pub struct hts_itr_t { 
	#[bitfield(name = "read_rest", ty = "c_uchar", bits = "0..=0")]
	#[bitfield(name = "finished", ty = "c_uchar", bits = "1..=1")]
	#[bitfield(name = "is_cram", ty = "c_uchar", bits = "2..=2")]
	#[bitfield(name = "nocoor", ty = "c_uchar", bits = "3..=3")]
	#[bitfield(name = "multi", ty = "c_uchar", bits = "4..=4")]
	#[bitfield(name = "dummy", ty = "u32", bits = "5..=31")]
	bfield: [u8; 4],
	_unused: [u8;0],
}
impl hts_itr_t {
	pub fn sam_itr_next<H: AsMut<htsFile>, B: AsMut<bam1_t>>(&mut self, mut fp: H, mut brec: B) -> SamReadResult {
		let p = brec.as_mut();
		match unsafe {
			if self.multi() != 0 {
				hts_itr_multi_next(fp.as_mut(), self, p as *mut bam1_t as *mut c_void)
			} else {
				hts_itr_next(if fp.as_mut().is_bgzf() != 0 { fp.as_mut().fp.bgzf } else { null_mut::<BGZF>() },
					self, p as *mut bam1_t as *mut c_void, fp.as_mut() as *mut htsFile as *mut c_void)
			}
		} {
			0..=c_int::MAX => SamReadResult::Ok,
			-1 => SamReadResult::EOF,
			_ => SamReadResult::Error,
		}
	}
	pub fn tbx_itr_next<H: AsMut<htsFile>, T: AsMut<tbx_t>>(&mut self, mut fp: H, mut tbx: T, kstr: &mut kstring_t) -> TbxReadResult {
		match unsafe {
			hts_itr_next(if fp.as_mut().is_bgzf() != 0 { fp.as_mut().fp.bgzf } else { null_mut::<BGZF>() },
				self, kstr as *mut _ as *mut c_void, tbx.as_mut() as *mut tbx_t as *mut c_void)
			} {
			0..=c_int::MAX => TbxReadResult::Ok,
			-1 => TbxReadResult::EOF,
			_ => TbxReadResult::Error,
		}
	}
}

#[repr(C)]
pub enum htsFormatCategory { UnknownCategory, SequenceData, VariantData, IndexFile, RegionList }
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum htsLogLevel { HTS_LOG_OFF, HTS_LOG_ERROR, HTS_LOG_WARNING = 3, HTS_LOG_INFO, HTS_LOG_DEBUG, HTS_LOG_TRACE }

#[repr(C)]
#[derive(PartialEq,Clone,Copy)]
pub enum htsExactFormat { 
	UnknownFormat, BinaryFormat, TextFormat, 
	Sam, Bam, Bai, Cram, Crai, Vcf, Bcf, Csi, Gzi, Tbi, Bed,
	HtsGet, EmptyFormat, FastaFormat, FastqFormat, FaiFormat, FqiFormat,
	HtsCrypt4GH }
#[repr(C)]
#[derive(PartialEq)]
pub enum htsCompression { NoCompression, Gzip, Bgzf, Custom, Bzip2Compression }
#[repr(C)]
pub struct htsFormatVersion {
	major: c_short,
	minor: c_short,
}
#[repr(C)]
pub struct htsFormat { 
	category: htsFormatCategory,
	format: htsExactFormat,
	version: htsFormatVersion,
	compression: htsCompression,
	compression_level: c_short,
	_unused: [u8; 0],
}

#[repr(C)]
struct hts_tpool { _unused: [u8; 0], }

#[repr(C)]
pub struct htsThreadPool {
	pool: NonNull<hts_tpool>,
	qsize: c_int,	
}

impl htsThreadPool {
	pub fn init(n: usize) -> Option<Self> { NonNull::new(unsafe{hts_tpool_init(n as c_int)}).map(|pool| Self{pool, qsize: 0 }) }
}

impl Drop for htsThreadPool {
	fn drop(&mut self) { unsafe{hts_tpool_destroy(self.pool.as_mut())} }
}

unsafe impl Sync for htsThreadPool {}
unsafe impl Send for htsThreadPool {}

#[repr(C)]
pub enum hts_fmt_option {
	HtsOptCompressionLevel = 100,
	HtsOptNThreads,
	HtsOptThreadPool,
	HtsOptCacheSize,
	HtsOptBlockSize,
}

#[link(name = "hts")]
extern "C" {
	fn hts_open(fn_: *const c_char, mode: *const c_char) -> *mut htsFile;
	fn hts_close(fp_: *mut htsFile) -> c_int;
	fn hts_set_threads(fp_ : *mut htsFile, t_: c_int) -> c_int;
	fn hts_get_format(fp_ : *const htsFile) -> *mut htsFormat;
	fn sam_index_load(fp_ : *mut htsFile, name: *const c_char) -> *mut hts_idx_t;
	fn hts_set_fai_filename(fp_ : *mut htsFile, fn_aux: *const c_char) -> c_int;
	fn hts_itr_destroy(iter: *mut hts_itr_t);
	fn sam_itr_queryi(idx: *const hts_idx_t, tid: c_int, start: HtsPos, end: HtsPos) -> *mut hts_itr_t; 
	fn sam_itr_regarray(idx: *const hts_idx_t, hdr: *mut sam_hdr_t, regarray: *const *const c_char, count: c_uint) -> *mut hts_itr_t; 
	fn hts_itr_multi_next(fp: *mut htsFile, itr: *mut hts_itr_t, r: *mut c_void) -> c_int;
	fn hts_itr_next(fp: *mut BGZF, itr: *mut hts_itr_t, r: *mut c_void, data: *mut c_void) -> c_int;
	fn bgzf_write(fp: *mut BGZF, data: *const c_void, length: size_t) -> ssize_t;
	fn bgzf_flush(fp: *mut BGZF) -> c_int;
	fn hwrite2(fp: *mut hFILE, srcv: *const c_void, total_bytes: size_t, ncopied: size_t) -> ssize_t; 
	fn hflush(fp: *mut hFILE) -> c_int;	
	fn hfile_set_blksize(fp: *mut hFILE, bufsize: size_t) -> c_int;	
	fn hts_idx_init(n: c_int, fmt: c_int, offset0: u64, min_shift: c_int, n_lvls: c_int) -> *mut hts_idx_t;
	fn hts_idx_destroy(idx: *mut hts_idx_t);
	fn hts_idx_push(idx: *mut hts_idx_t, tid: c_int, beg: HtsPos, end: HtsPos, offset: u64, is_mapped: c_int) -> c_int;
	fn hts_idx_finish(idx: *mut hts_idx_t, final_offset: u64) -> c_int;
	fn hts_idx_set_meta(idx: *mut hts_idx_t, l_meta: u32, meta: *const u8, is_copy: c_int) -> c_int;
	fn hts_idx_save_as(idx: *mut hts_idx_t, fname: *const c_char, fnidx: *const c_char, fmt: c_int) -> c_int;
	fn hts_set_opt(fp: *mut htsFile, opt: hts_fmt_option, ...) -> c_int;
	fn hts_tpool_init(n: c_int) -> *mut hts_tpool;
	fn hts_tpool_destroy(p: *mut hts_tpool);
	pub fn hts_set_log_level(level: htsLogLevel);
}

pub struct HtsFile {
	inner: NonNull<htsFile>,
	phantom: PhantomData<htsFile>,
	name: String,
}

impl Deref for HtsFile {
	type Target = htsFile;
	#[inline]
	fn deref(&self) -> &htsFile { unsafe{self.inner.as_ref()} }	
}

impl DerefMut for HtsFile {
	#[inline]
	fn deref_mut(&mut self) -> &mut htsFile {unsafe{ self.inner.as_mut() }}
}

impl AsRef<htsFile> for HtsFile {
	#[inline]
	fn as_ref(&self) -> &htsFile { self}	
}

impl AsMut<htsFile> for HtsFile {
	#[inline]
	fn as_mut(&mut self) -> &mut htsFile { self}	
}

unsafe impl Sync for HtsFile {}
unsafe impl Send for HtsFile {}

impl Drop for HtsFile {
	fn drop(&mut self) {
		unsafe { hts_close(self.as_mut()) };
	}
}

impl HtsFile {
	pub fn new<S: AsRef<str>>(name: S, mode: &str) -> io::Result<Self> {
		let name = name.as_ref();
		match NonNull::new(unsafe{ hts_open(get_cstr(name).as_ptr(), get_cstr(mode).as_ptr())}) {
			None =>	Err(hts_err(format!("Couldn't open file {} with mode {}", name, mode))),
			Some(fptr) => Ok(HtsFile{inner: fptr, phantom: PhantomData, name: name.to_owned()}), 
		}
	}
	pub fn name(&self) -> &str { &self.name }
	pub fn sam_index_load(&mut self) -> io::Result<HtsIndex> {
		match NonNull::new(unsafe { sam_index_load(self.as_mut(), get_cstr(&self.name).as_ptr()) }) {
			None => Err(hts_err(format!("Couldn't load SAM index for file {}", self.name))),
			Some(idx) => Ok(HtsIndex{inner: idx, phantom: PhantomData}),
		}
	}	
}

fn hwrite(fp: &mut hFILE, buffer: *const c_void, nbytes: size_t) -> ssize_t {
	let nbytes1 = nbytes as isize;
	if fp.mobile() != 0 {
		let n = unsafe{fp.limit.offset_from(fp.begin)};
		if n < nbytes1 {
			let s = unsafe{fp.limit.offset_from(fp.buffer)} + nbytes1;
			unsafe{hfile_set_blksize(fp as *mut hFILE, s as size_t)};
			fp.end = fp.limit;
		}
	}
	let mut n = unsafe{fp.limit.offset_from(fp.begin)};
	if nbytes1 >= n && fp.begin == fp.buffer {
		unsafe{hwrite2(fp as *mut hFILE, buffer, nbytes, n as size_t)}		
	} else {
		if n > nbytes1 { n = nbytes1 }
		unsafe{libc::memcpy(fp.begin as *mut c_void, buffer, n as size_t); }
		fp.begin = unsafe {fp.begin.add(n as usize)};
		if n == nbytes1 { n } else { unsafe {hwrite2(fp as *mut hFILE, buffer, nbytes, n as size_t) }}	
	}
}

impl io::Write for HtsFile {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> { self.as_mut().write(buf) }
	fn flush(&mut self) -> io::Result<()> { self.as_mut().flush() }
}

pub struct HtsIndex {
	inner: NonNull<hts_idx_t>,	
	phantom: PhantomData<hts_idx_t>,
}

impl Deref for HtsIndex {
	type Target = hts_idx_t;
	#[inline]
	fn deref(&self) -> &hts_idx_t { unsafe{self.inner.as_ref()} }	
}

impl DerefMut for HtsIndex {
	#[inline]
	fn deref_mut(&mut self) -> &mut hts_idx_t {unsafe{ self.inner.as_mut() }}
}

impl AsRef<hts_idx_t> for HtsIndex {
	#[inline]
	fn as_ref(&self) -> &hts_idx_t { self}	
}

impl AsMut<hts_idx_t> for HtsIndex {
	#[inline]
	fn as_mut(&mut self) -> &mut hts_idx_t { self}	
}

impl Drop for HtsIndex {
	fn drop(&mut self) {
		unsafe { hts_idx_destroy(self.as_mut()) };
	}
}

unsafe impl Sync for HtsIndex {}
unsafe impl Send for HtsIndex {}

impl HtsIndex {
	pub fn init(n: usize, fmt: c_int, offset0: u64, min_shift: usize, n_lvls: usize) -> io::Result<HtsIndex> {
		match NonNull::new(unsafe { hts_idx_init(n as c_int, fmt, offset0, min_shift as c_int, n_lvls as c_int) }) {
			Some(p) => Ok(HtsIndex{inner: p, phantom: PhantomData}),
			None => Err(hts_err("Failed to initialize hts_idx_t".to_string())),
		}
	}
	fn inner(&self) -> &hts_idx_t { unsafe{self.inner.as_ref()} }
	fn inner_mut(&mut self) -> &mut hts_idx_t { unsafe{self.inner.as_mut()} }
	pub fn sam_itr_queryi(&self, tid: isize, start: usize, end: usize) -> io::Result<HtsItr> {
		HtsItr::new(unsafe{ sam_itr_queryi(self.inner(), tid as libc::c_int, start as HtsPos, end as HtsPos)}).ok_or_else(|| hts_err("Failed to obtain sam iterator".to_string()))
	}
	pub fn sam_itr_regarray<H: AsMut<sam_hdr_t>>(&self, mut hdr: H, regions: &[String]) -> io::Result<HtsItr> {
		let count = regions.len();
		if count == 0 { return self.sam_itr_queryi(HTS_IDX_START as isize, 0, 0) }
		// We need to do this in 2 stages: generate and array of CStrings and then an array of ptrs to the CStrings
		// otherwise the CStrings go out of scope before they are used and the ptrs point to garbage
		let carray: Vec<CString> = regions.iter().map(get_cstr).collect();
		let parray: Vec<*const c_char> = carray.iter().map(|cs| cs.as_ptr()).collect();
		HtsItr::new(unsafe{ sam_itr_regarray(self.inner(), hdr.as_mut(), parray.as_ptr(), count as c_uint)}).ok_or_else(|| hts_err("Failed to obtain sam iterator".to_string()))
	}
	pub fn push(&mut self, tid: usize, beg: usize, end: usize, offset: u64, is_mapped: bool) -> io::Result<()> {
		if unsafe{hts_idx_push(
			self.inner_mut(), tid as c_int, beg as HtsPos, end as HtsPos, offset, if is_mapped { 1 } else { 0 }
		)} == 0 { Ok(()) }
		else { Err(hts_err("Error returned from hts_idx_push".to_string()))} 
	}
	pub fn finish(&mut self, final_offset: u64) -> io::Result<()> {
		if unsafe{hts_idx_finish(self.inner_mut(), final_offset)} == 0 { Ok(()) } 
		else { Err(hts_err("Error returned from hts_idx_finish".to_string()))} 
	}
	pub fn set_meta(&mut self, meta: &[u8]) -> io::Result<()> {
		if unsafe{hts_idx_set_meta(self.inner_mut(), meta.len() as u32, meta.as_ptr(), 1)} == 0 { Ok(()) }
		else { Err(hts_err("Error returned from hts_idx_set_meta".to_string()))} 
	}
	pub fn save_as<S: AsRef<str>>(&mut self, fname: S, fmt: c_int) -> io::Result<()> {
		if unsafe{hts_idx_save_as(self.inner_mut(), get_cstr(fname.as_ref()).as_ptr(), std::ptr::null() as *const c_char, fmt)} == 0 { Ok(()) }
		else { Err(hts_err("Error returned from hts_idx_save_as".to_string()))}
	}
}

impl htsFormat {
	pub fn format(&self) -> htsExactFormat { self.format }
	pub fn is_compressed(&self) -> bool {
		self.compression == htsCompression::Bgzf || self.format() == htsExactFormat::Cram
	}	
}

pub struct HtsItr {
	inner: NonNull<hts_itr_t>,
	phantom: PhantomData<hts_itr_t>,
}

impl Deref for HtsItr {
	type Target = hts_itr_t;
	#[inline]
	fn deref(&self) -> &hts_itr_t { unsafe{self.inner.as_ref()} }	
}

impl DerefMut for HtsItr {
	#[inline]
	fn deref_mut(&mut self) -> &mut hts_itr_t {unsafe{ self.inner.as_mut() }}
}

impl AsRef<hts_itr_t> for HtsItr {
	#[inline]
	fn as_ref(&self) -> &hts_itr_t { self}	
}

impl AsMut<hts_itr_t> for HtsItr {
	#[inline]
	fn as_mut(&mut self) -> &mut hts_itr_t { self}	
}

unsafe impl Sync for HtsItr {}
unsafe impl Send for HtsItr {}

impl Drop for HtsItr {
	fn drop(&mut self) { unsafe { hts_itr_destroy(self.as_mut()) }}
}

impl HtsItr {
	pub fn new(itr: *mut hts_itr_t) -> Option<Self> { NonNull::new(itr).map(|p| HtsItr{ inner: p, phantom: PhantomData}) }
}

use std::io::{self, ErrorKind, Error};
use std::ptr::{null_mut, NonNull, null};
use std::ffi::{CString, c_void, CStr};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use libc::{c_char, c_int, c_short, c_uint, c_uchar, size_t, ssize_t, off_t};
use c2rust_bitfields::BitfieldStruct;
use super::{hts_err, get_cstr, SamReadResult, sam_hdr_t, bam1_t, kstring_t};

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
	buffer: *mut c_char,
	begin: *mut c_char,
	end: *mut c_char,
	limit: *mut c_char,
	backend: *const c_void,
	offset: off_t,
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
	fn flush(&mut self) -> io::Result<()> { Ok(())}
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
	pub fn tbx_itr_next<H: AsMut<htsFile>, T: AsMut<tbx_t>>(&mut self, mut fp: H, mut tbx: T, mut kstr: kstring_t) -> TbxReadResult {
		match unsafe {
			hts_itr_next(if fp.as_mut().is_bgzf() != 0 { fp.as_mut().fp.bgzf } else { null_mut::<BGZF>() },
				self, &mut kstr as *mut _ as *mut c_void, tbx.as_mut() as *mut tbx_t as *mut c_void)
			} {
			0..=c_int::MAX => TbxReadResult::Ok(kstr),
			-1 => TbxReadResult::EOF,
			_ => TbxReadResult::Error,
		}
	}
}
#[repr(C)]
pub struct bgzidx_t { _unused: [u8; 0], }
#[repr(C)]
#[derive(BitfieldStruct)]
pub struct BGZF {
	#[bitfield(name = "errcode", ty = "u16", bits = "0..=15")]
	#[bitfield(name = "reserved", ty = "c_uchar", bits = "16..=16")]
	#[bitfield(name = "is_write", ty = "c_uchar", bits = "17..=17")]
	#[bitfield(name = "no_eof_block", ty = "c_uchar", bits = "18..=18")]
	#[bitfield(name = "is_be", ty = "c_uchar", bits = "19..=19")]
	#[bitfield(name = "compress_level", ty = "u16", bits = "20..=28")]
	#[bitfield(name = "last_block_eof", ty = "c_uchar", bits = "29..=29")]
	#[bitfield(name = "is_compressed", ty = "c_uchar", bits = "30..=30")]
	#[bitfield(name = "is_gzip", ty = "c_uchar", bits = "31..=31")]
	bfield: [u8; 4],
	cache_size: c_int,
	block_length: c_int,
	block_clength: c_int,
	block_offset: c_int,
	block_address: i64,
	uncompressed_address: i64,
	uncompressed_block: *mut c_void,
	compressed_block: *mut c_void,
	cache: *mut c_void,
	fp: *mut hFILE,
	mt: *mut c_void,
	idx: *mut bgzidx_t,
	idx_build_otf: c_int,
	z_stream_s: *mut c_void,
	seeked: i64,
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
pub struct htsThreadPool { _unused: [u8; 0], }

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

#[link(name = "hts")]
extern "C" {
	fn hts_open(fn_: *const c_char, mode: *const c_char) -> *mut htsFile;
	fn hts_close(fp_: *mut htsFile) -> c_int;
	fn hts_set_threads(fp_ : *mut htsFile, t_: c_int) -> c_int;
	fn hts_get_format(fp_ : *const htsFile) -> *mut htsFormat;
	fn sam_index_load(fp_ : *mut htsFile, name: *const c_char) -> *mut hts_idx_t;
	fn hts_set_fai_filename(fp_ : *mut htsFile, fn_aux: *const c_char) -> c_int;
	fn hts_itr_destroy(iter: *mut hts_itr_t);
	fn hts_itr_query(idx: *const hts_idx_t, tid: c_int, beg: HtsPos, end: HtsPos,
		readrec: unsafe extern "C" fn (*mut BGZF, *mut c_void, *mut c_void, *mut c_int, *mut HtsPos, *mut HtsPos) -> c_int) -> *mut hts_itr_t;
	fn sam_itr_queryi(idx: *const hts_idx_t, tid: c_int, start: HtsPos, end: HtsPos) -> *mut hts_itr_t; 
	fn sam_itr_regarray(idx: *const hts_idx_t, hdr: *mut sam_hdr_t, regarray: *const *const c_char, count: c_uint) -> *mut hts_itr_t; 
	fn hts_itr_multi_next(fp: *mut htsFile, itr: *mut hts_itr_t, r: *mut c_void) -> c_int;
	fn hts_itr_next(fp: *mut BGZF, itr: *mut hts_itr_t, r: *mut c_void, data: *mut c_void) -> c_int;
	fn tbx_index_load3(fname: *const c_char, fnidx: *const c_char, flags: c_int) -> *mut tbx_t;
	fn tbx_seqnames(tbx: *const tbx_t, n: *mut c_int) -> *mut *const c_char;
	fn tbx_readrec(fp: *mut BGZF, tbxv: *mut c_void, sv: *mut c_void, tid: *mut c_int, beg: *mut HtsPos, end: *mut HtsPos) -> c_int;
	fn tbx_destroy(tbx: *mut tbx_t);
	fn bgzf_write(fp: *mut BGZF, data: *const c_void, length: size_t) -> ssize_t;
	fn bgzf_seek(fp: *mut BGZF, pos: i64, whence: c_int) -> i64;
	fn bgzf_read_block(fp: *mut BGZF) -> c_int;
	fn hwrite2(fp: *mut hFILE, srcv: *const c_void, total_bytes: size_t, ncopied: size_t) -> ssize_t; 	
	fn hfile_set_blksize(fp: *mut hFILE, bufsize: size_t) -> c_int;	
	fn hts_idx_init(n: c_int, fmt: c_int, offset0: u64, min_shift: c_int, n_lvls: c_int) -> *mut hts_idx_t;
	fn hts_idx_destroy(idx: *mut hts_idx_t);
	fn hts_idx_push(idx: *mut hts_idx_t, tid: c_int, beg: HtsPos, end: HtsPos, offset: u64, is_mapped: c_int) -> c_int;
	fn hts_idx_finish(idx: *mut hts_idx_t, final_offset: u64) -> c_int;
	fn hts_idx_set_meta(idx: *mut hts_idx_t, l_meta: u32, meta: *const u8, is_copy: c_int) -> c_int;
	fn hts_idx_save_as(idx: *mut hts_idx_t, fname: *const c_char, fnidx: *const c_char, fmt: c_int) -> c_int;
	pub fn tbx_index_build(fname: *const c_char, min_shift: c_int, conf: *const tbx_conf_t) -> c_int;
	pub fn hts_set_log_level(level: htsLogLevel);
}

impl BGZF {
	pub fn tell(&self) -> i64 { (self.block_address << 16) | ((self.block_offset & 0xffff) as i64)}
	pub fn htell(&self) -> off_t {
		if self.mt.is_null() {
			let fp = unsafe{&*self.fp};
			fp.offset + (unsafe{fp.begin.offset_from(fp.buffer)} as off_t)
		} else { panic!("Multithreaded htell() not supported")}	
	}
	pub fn seek(&mut self, pos: i64) -> i64 {unsafe{bgzf_seek(self, pos, libc::SEEK_SET as c_int)}}
	pub fn getline(&mut self, delim: u8, s: &mut Vec<u8>) -> io::Result<usize> {
		s.clear();
		let mut state = 0;
		loop {
			if self.block_offset >= self.block_length {
				if unsafe{bgzf_read_block(self)} != 0 {
					state = -2;
					break;
				}
				if self.block_length == 0 {
					state = -1;
					break;
				}
			}
			let buf = unsafe{std::slice::from_raw_parts(self.uncompressed_block as *const u8, self.block_length as usize)};
			let mut l = 0;
			for c in &buf[self.block_offset as usize..] {
				l += 1;
				if *c == delim { 
					state = 1;
					break;
				}
				s.push(*c);
			}
			self.block_offset += l;
			if self.block_offset >= self.block_length {
				let fp = unsafe{&*self.fp};
				self.block_address = fp.offset + (unsafe{fp.begin.offset_from(fp.buffer)} as off_t);
				self.block_offset = 0;
				self.block_length = 0;
			}
			if state != 0 { break }
		}
		if state < 0 {
			if s.is_empty() {
				if state == -1 { Err(Error::new(ErrorKind::UnexpectedEof, "EOF")) }
				else { Err(hts_err("Error uncompressing data".to_string()))	} 
			} else {
				s.clear();
				Err(Error::new(ErrorKind::UnexpectedEof, "Incomplete line"))				
			}
		} else {
			self.uncompressed_address += 1 + s.len() as i64;	
			if s.is_empty() { Ok (0) } // Empty line
			else {
				if delim == b'\n' && s[s.len() - 1] == b'\r' { s.pop(); }			
				Ok(s.len())
			}
		}
	}
	pub fn clear_eof(&mut self) {
		self.set_last_block_eof(0);
		self.set_no_eof_block(0);
		let fp = unsafe{&mut *self.fp};
		fp.set_at_eof(0);
		fp.begin = fp.buffer;
		fp.end = fp.buffer;
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
	Ok(kstring_t),
	EOF,	
	Error,
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
	fn flush(&mut self) -> io::Result<()> { Ok(())}
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
	fn new(itr: *mut hts_itr_t) -> Option<Self> { NonNull::new(itr).map(|p| HtsItr{ inner: p, phantom: PhantomData}) }
}

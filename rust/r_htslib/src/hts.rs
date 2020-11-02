use std::io;
use std::ptr::{null_mut, NonNull};
use std::ffi::{CString, c_void};
use libc::{c_char, c_int, c_short, c_uint, c_uchar};
use c2rust_bitfields::BitfieldStruct;
use super::{hts_err, get_cstr, SamHeader, SamReadResult, sam_hdr_t, bam1_t, BamRec, kstring_t};

pub type HtsPos = i64;

pub const HTS_IDX_NOCOOR: HtsPos = -2;
pub const HTS_IDX_START: HtsPos = -3;
pub const HTS_IDX_REST: HtsPos = -4;
pub const HTS_IDX_NONE: HtsPos = -5;

pub const FT_UNKN: u32 = 0;
pub const FT_GZ: u32 = 1;
pub const FT_VCF: u32 = 2;
pub const FT_VCF_GZ: u32 = FT_GZ | FT_VCF;
pub const FT_BCF: u32 = 4;
pub const FT_BCF_GZ: u32 = FT_GZ | FT_BCF;
pub const FT_STDIN: u32 = 8;

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
	fp: *mut c_void,
	_unused: [u8; 0], 
}
#[repr(C)]
pub struct hts_idx_t { _unused: [u8; 0], }
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
#[repr(C)]
pub struct BGZF { _unused: [u8; 0], }
#[repr(C)]
pub enum htsFormatCategory { UnknownCategory, SequenceData, VariantData, IndexFile, RegionList }
#[repr(C)]
#[derive(PartialEq)]
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
}

pub struct HtsFile {
	inner: NonNull<htsFile>,
	name: String,
}

unsafe impl Sync for HtsFile {}
unsafe impl Send for HtsFile {}

impl Drop for HtsFile {
	fn drop(&mut self) {
		unsafe { hts_close(self.inner_mut()) };
	}
}

impl HtsFile {
	pub fn new<S: AsRef<str>>(name: S, mode: &str) -> io::Result<Self> {
		let name = name.as_ref();
		match NonNull::new(unsafe{ hts_open(get_cstr(name).as_ptr(), get_cstr(mode).as_ptr())}) {
			None =>	Err(hts_err(format!("Couldn't open file {} with mode {}", name, mode))),
			Some(fptr) => Ok(HtsFile{inner: fptr, name: name.to_owned()}), 
		}
	}
	pub(crate) fn name(&self) -> &str { &self.name }
	pub(crate) fn inner(&self) -> &htsFile { unsafe{self.inner.as_ref()} }
	pub(crate) fn inner_mut(&mut self) -> &mut htsFile { unsafe{ self.inner.as_mut() }} 

	pub fn set_threads(&mut self, t: usize) -> io::Result<()> {
		let ret = unsafe{hts_set_threads(self.inner_mut(), t as c_int)};
		if ret != 0 { Err(hts_err(format!("Failed to set additional threads to file {}", self.name))) } 
		else { Ok(()) }
	}
	pub fn format(&self) -> HtsFormat {	
		HtsFormat{inner: NonNull::new(unsafe{hts_get_format(self.inner())}).expect("hts_get_format returned NULL")} 
	}
	pub fn sam_index_load(&mut self) -> io::Result<HtsIndex> {
		match NonNull::new(unsafe { sam_index_load(self.inner_mut(), get_cstr(&self.name).as_ptr()) }) {
			None => Err(hts_err(format!("Couldn't load SAM index for file {}", self.name))),
			Some(idx) => Ok(HtsIndex{inner: idx}),
		}
	}
	pub fn set_fai_filename<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> {
		let name = name.as_ref();
		let ret = unsafe { hts_set_fai_filename(self.inner_mut(), get_cstr(name).as_ptr()) };
		if ret != 0 { Err(hts_err(format!("Failed to attach reference index {} to file {}", name, self.name))) } 
		else { Ok(()) }
	}

}

pub struct HtsIndex {
	inner: NonNull<hts_idx_t>,	
}

unsafe impl Sync for HtsIndex {}
unsafe impl Send for HtsIndex {}

impl HtsIndex {
	fn inner(&self) -> &hts_idx_t { unsafe{self.inner.as_ref()} }
	pub fn sam_itr_queryi(&self, tid: isize, start: usize, end: usize) -> io::Result<HtsItr> {
		let it = NonNull::new(unsafe{ sam_itr_queryi(self.inner(), tid as libc::c_int, start as HtsPos, end as HtsPos) });
		if let Some(itr) = it { Ok(HtsItr{inner: itr}) } else { Err(hts_err("Failed to obtain sam iterator".to_string())) }
	}
	pub fn sam_itr_regarray(&self, hdr: &mut SamHeader, regions: &[String]) -> io::Result<HtsItr> {
		let count = regions.len();
		if count == 0 { return self.sam_itr_queryi(HTS_IDX_START as isize, 0, 0) }
		// We need to do this in 2 stages: generate and array of CStrings and then an array of ptrs to the CStrings
		// otherwise the CStrings go out of scope before they are used and the ptrs point to garbage
		let carray: Vec<CString> = regions.iter().map(get_cstr).collect();
		let parray: Vec<*const c_char> = carray.iter().map(|cs| cs.as_ptr()).collect();
		let it = NonNull::new(unsafe{ sam_itr_regarray(self.inner(), hdr.inner_mut(), parray.as_ptr(), count as c_uint)});
		if let Some(itr) = it { Ok(HtsItr{inner: itr}) } else { Err(hts_err("Failed to obtain sam iterator".to_string())) }
	}
}

pub struct HtsFormat {
	inner: NonNull<htsFormat>,	
}

impl HtsFormat {
	pub fn is_compressed(&self) -> bool {
		let format = unsafe { self.inner.as_ref() };
		format.compression == htsCompression::Bgzf || format.format == htsExactFormat::Cram
	}	
}

pub struct HtsItr {
	inner: NonNull<hts_itr_t>,	
}

unsafe impl Sync for HtsItr {}
unsafe impl Send for HtsItr {}

impl Drop for HtsItr {
	fn drop(&mut self) { unsafe { hts_itr_destroy(self.inner.as_ptr()) }; }
}

impl HtsItr {
	pub fn sam_itr_next(&mut self, fp: &mut HtsFile, mut brec: BamRec) -> SamReadResult {
		let p = brec.inner_mut();
		match unsafe {
			if (*self.inner.as_ref()).multi() != 0 {
				hts_itr_multi_next(fp.inner_mut(), self.inner.as_ptr(), p as *mut bam1_t as *mut c_void)
			} else {
				hts_itr_next(if fp.inner().is_bgzf() != 0 { fp.inner().fp as *mut BGZF } else { null_mut::<BGZF>() },
					self.inner.as_ptr(), p as *mut bam1_t as *mut c_void, fp.inner_mut() as *mut htsFile as *mut c_void)
			}
		} {
			0..=c_int::MAX => SamReadResult::Ok(brec),
			-1 => SamReadResult::EOF,
			_ => SamReadResult::Error,
		}
	}
}
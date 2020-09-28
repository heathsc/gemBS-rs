use std::ffi;

pub use rust_htslib::htslib::{FT_VCF, FT_BCF, FT_VCF_GZ, FT_BCF_GZ, FT_GZ, FT_UNKN, BCF_DT_ID, BCF_DT_CTG, BCF_DT_SAMPLE};

pub mod hts;
pub use hts::*;
pub mod vcf;
pub use vcf::{VcfHeader, VcfFile};
pub mod sam;
pub use sam::{SamHeader, SamFile, SamItr, RegionItr};
pub mod faidx;
pub use faidx::{Faidx, faidx_load};

fn get_cstr<S: AsRef<str>>(s: S) -> ffi::CString { ffi::CString::new(s.as_ref().as_bytes()).unwrap() }

fn from_cstr<'a>(cstr: *const i8) -> &'a str {
	if cstr.is_null() { panic!("sam_hdr_tid2name() returned NULL"); }
	unsafe { ffi::CStr::from_ptr(cstr).to_str().expect("Reference name not UTF8") }
}

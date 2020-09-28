use std::io;
use std::ptr::NonNull;

use rust_htslib::htslib;
use crate::config::new_err;
use crate::defs::CtgRegion;

use super::{get_cstr, from_cstr, HtsFile, HtsIndex, HtsFormat};

pub struct SamHeader {
	inner: *mut htslib::sam_hdr_t,	
}

impl Drop for SamHeader {
	fn drop(&mut self) {
		unsafe { htslib::sam_hdr_destroy(self.inner) };
	}
}

impl SamHeader {
	pub fn nref(&self) -> usize { 
		let l = unsafe { htslib::sam_hdr_nref(self.inner) };
		l as usize
	}
	
	fn check_idx(&self, i: usize) { if i >= self.nref() { panic!("Reference ID {} out of range", i); }}
	
	pub fn tid2name(&self, i: usize) -> &str {
		self.check_idx(i);
		from_cstr(unsafe { htslib::sam_hdr_tid2name(self.inner, i as libc::c_int) })
	}
	
	pub fn tid2len(&self, i: usize) -> usize {
		self.check_idx(i);
		let len = unsafe { htslib::sam_hdr_tid2len(self.inner, i as libc::c_int) };
		len as usize
	}
	
	pub fn name2tid<S: AsRef<str>>(&self, cname: S) -> Option<usize> {
		let cname = cname.as_ref();
		let tid = unsafe{ htslib::sam_hdr_name2tid(self.inner, get_cstr(cname).as_ptr())};
		if tid < 0 { None } else { Some(tid as usize) }
	}

	pub fn text(&self) -> &str {
		from_cstr(unsafe { htslib::sam_hdr_str(self.inner)})
	}
}

pub struct SamFile {
	file: HtsFile, 
	index: HtsIndex,
	hdr: SamHeader,
	regions: Vec<CtgRegion>,
}

impl SamFile {
	pub fn new<S: AsRef<str>>(name: S) -> io::Result<Self> {
		let name = name.as_ref();
		let mut file = HtsFile::new(name, "r")?;
		let index = file.sam_index_load()?;
		let hdr_ptr = unsafe { htslib::sam_hdr_read(file.inner()) };
		if hdr_ptr.is_null() { return Err(new_err(format!("Failed to load header from {}", name))) }
		let hdr = SamHeader{inner: hdr_ptr};
		Ok(Self{file, index, hdr, regions: Vec::new()})
	}
	pub fn nref(&self) -> usize { self.hdr.nref() }
	pub fn tid2name(&self, i: usize) -> &str { self.hdr.tid2name(i) }
	pub fn tid2len(&self, i: usize) -> usize { self.hdr.tid2len(i) }
	pub fn name2tid<S: AsRef<str>>(&self, cname: S) -> Option<usize> { self.hdr.name2tid(cname) }
	pub fn text(&self) -> &str { self.hdr.text() }
	pub fn format(&self) -> HtsFormat {	self.file.format() }
	pub fn set_threads(&mut self, t: usize) -> io::Result<()> { self.file.set_threads(t) }
	pub fn set_fai_filename<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> { self.file.set_fai_filename(name) }
	pub fn add_regions(&mut self, creg: &mut[CtgRegion]) { self.regions.extend_from_slice(creg); }
	pub fn region_iter(&self) -> io::Result<RegionItr> { Ok(RegionItr{hdr: &self.hdr, index: &self.index, regions: &self.regions, ix: 0})}	
}


pub struct SamItr {
	itr: NonNull<htslib::hts_itr_t>,
}

pub struct RegionItr<'a> {
	hdr: &'a SamHeader,
	index: &'a HtsIndex,
	regions: &'a Vec<CtgRegion>,
	ix: usize,	
}

impl <'a> RegionItr<'a> {
	fn get_region_itr(&self, reg: Option<&CtgRegion>) -> SamItr {
		let it = NonNull::new(unsafe{ 
			if let Some(r) = reg { htslib::sam_itr_queryi(self.index.inner(), r.sam_tid as libc::c_int, r.start as htslib::hts_pos_t, r.stop as htslib::hts_pos_t)}
		 	else { htslib::sam_itr_queryi(self.index.inner(), htslib::HTS_IDX_START, 0, 0) }
		});
		if let Some(itr) = it {
			if let Some(r) = reg { trace!("Got SAM iterator for {}:{}-{}", self.hdr.tid2name(r.sam_tid), r.start + 1, r.stop + 1); } 
			else { trace!("Got SAM iterator for entire file"); }
			SamItr{itr}
		} else { panic!("Failed to obtain sam iterator"); }
	}
} 

impl <'a>Iterator for RegionItr<'a> {
	type Item = SamItr;
	
	fn next(&mut self) -> Option<SamItr> {
		if self.regions.is_empty() {
			if self.ix == 0 { 
				self.ix = 1;
				Some(self.get_region_itr(None))
			} else { None }
		} else if self.ix < self.regions.len() {
			let reg = &self.regions[self.ix];
			self.ix += 1;
			Some(self.get_region_itr(Some(reg)))
		} else { None }
	}	
}

pub struct BamRec {
	inner: *mut htslib::bam1_t,
}

impl Drop for BamRec {
	fn drop(&mut self) {
		unsafe { htslib::bam_destroy1(self.inner) };
	}
}

impl BamRec {
	pub fn new() -> io::Result<Self> { 
		let b = unsafe { htslib::bam_init1() };
		if b.is_null() { Err(new_err("Failed to allocate new BamRec".to_string())) }
		else { Ok(BamRec{inner: b}) }
	}
}

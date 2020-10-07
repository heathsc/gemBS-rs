use std::io;

use r_htslib::*;
use crate::defs::CtgRegion;

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
		let hdr = SamHeader::read(&mut file)?;
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

pub struct RegionItr<'a> {
	hdr: &'a SamHeader,
	index: &'a HtsIndex,
	regions: &'a Vec<CtgRegion>,
	ix: usize,	
}

impl <'a> RegionItr<'a> {
	fn get_region_itr(&self, reg: Option<&CtgRegion>) -> HtsItr {
		if let Some(r) = reg { self.index.sam_itr_queryi(r.sam_tid as isize, r.start, r.stop)}
		else { self.index.sam_itr_queryi(HTS_IDX_START as isize, 0, 0) }
	}
} 

impl <'a>Iterator for RegionItr<'a> {
	type Item = HtsItr;
	
	fn next(&mut self) -> Option<HtsItr> {
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


use std::io;

use r_htslib::*;
use crate::defs::CtgRegion;

pub struct SamFile {
	pub file: HtsFile, 
	pub index: HtsIndex,
	pub hdr: SamHeader,
	pub itr: Option<HtsItr>,
}

impl SamFile {
	pub fn new<S: AsRef<str>>(name: S) -> io::Result<Self> {
		let name = name.as_ref();
		let mut file = HtsFile::new(name, "r")?;
		let index = file.sam_index_load()?;
		let hdr = SamHeader::read(&mut file)?;
		Ok(Self{file, index, hdr, itr: None})
	}
	pub fn nref(&self) -> usize { self.hdr.nref() }
	pub fn tid2name(&self, i: usize) -> &str { self.hdr.tid2name(i) }
	pub fn tid2len(&self, i: usize) -> usize { self.hdr.tid2len(i) }
	pub fn name2tid<S: AsRef<str>>(&self, cname: S) -> Option<usize> { self.hdr.name2tid(cname) }
	pub fn text(&self) -> &str { self.hdr.text() }
	pub fn format(&self) -> HtsFormat {	self.file.format() }
	pub fn set_threads(&mut self, t: usize) -> io::Result<()> { self.file.set_threads(t) }
	pub fn set_fai_filename<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> { self.file.set_fai_filename(name) }
	pub fn set_itr(&mut self) -> io::Result<()> {
		self.itr = Some(self.index.sam_itr_queryi(HTS_IDX_START as isize, 0, 0)?);
		Ok(()) 
	}	 
	pub fn set_region_itr(&mut self, regions: &[CtgRegion]) -> io::Result<()> { 
		let reg_str: Vec<String> = regions.iter().map(|r| format!("{}:{}-{}", self.hdr.tid2name(r.sam_tid), r.start, r.stop)).collect();
		self.itr = Some(self.index.sam_itr_regarray(&mut self.hdr, &reg_str)?);
		Ok(())
	}
	pub fn get_next(&self, brec: &mut BamRec) -> SamReadResult {
		if let Some(itr) = &self.itr {
			itr.sam_itr_next(&self.file, brec)
		} else { SamReadResult::Error }
	}
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BSStrand {Unknown, StrandC2T, StrandG2A, Unconverted}

impl BSStrand {
	pub fn get_num(&self) -> u16 {
		match self {
			BSStrand::Unknown => 0,
			BSStrand::StrandC2T => 1,
			BSStrand::StrandG2A => 2,
			BSStrand::Unconverted => 3,
		}
	}
}


#[derive(PartialEq)]
pub enum Aligner { Unknown, GEM, Bowtie, Novoalign, BSMap, BWAMeth }

pub fn get_bs_strand(b: &BamRec) -> (BSStrand, Option<&[u8]>) {
	let mut sa_tag = None;
	let mut strand = BSStrand::Unconverted;
	if let Some(itr) = b.get_aux_iter() {
		for tag in itr {
			let aligner = {
				if tag[0] == b'Z' {
					if tag[1] == b'B' { Aligner::Novoalign }
					else if tag[1] == b'S' { Aligner::BSMap }
					else {Aligner::Unknown}
				} else if tag[0] == b'X' {
					if tag[1] == b'G' { Aligner::Bowtie }
					else if tag[1] == b'B' { Aligner::GEM }
					else { Aligner::Unknown }
				} else if tag[0] == b'Y' && tag[1] == b'D' { Aligner::BWAMeth }
				else { Aligner::Unknown } 
			};
			if aligner != Aligner::Unknown {
				match tag[2] {
					b'A' if aligner == Aligner::GEM => {
						if tag[3] == b'C' { strand = BSStrand::StrandC2T }
						else if tag[3] == b'G' { strand = BSStrand::StrandG2A }
					},
					b'Z' => match aligner {
						Aligner::Bowtie | Aligner::Novoalign => {
							if tag[3] == b'C' { strand = BSStrand::StrandC2T }
							else if tag[3] == b'G' { strand = BSStrand::StrandG2A }						
						},
						Aligner::BSMap => {
							if tag[3] == b'+' { strand = BSStrand::StrandC2T }
							else if tag[3] == b'-' { strand = BSStrand::StrandG2A }													
						}
						Aligner::BWAMeth => {
							if tag[3] == b'f' { strand = BSStrand::StrandC2T }
							else if tag[3] == b'r' { strand = BSStrand::StrandG2A }													
						}
						_ => (),
					},
					_ => (),
				}
			} else if tag[0] == b'S' && tag[1] == b'A' && tag[2] == b'Z' { sa_tag = Some(&tag[3..]) }
			if strand != BSStrand::Unconverted && sa_tag.is_some() { break }
		}
	}
	(strand, sa_tag)
}

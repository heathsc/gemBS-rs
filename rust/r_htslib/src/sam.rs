use std::{io, fmt, ptr};
use std::ptr::NonNull;
use std::str::FromStr;
use std::convert::TryInto;
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use libc::{c_char, c_int, size_t};
use super::{hts_err, get_cstr, from_cstr, htsFile, HtsPos};

pub const BAM_FPAIRED: u16 = 1;
pub const BAM_FPROPER_PAIR: u16 = 2;
pub const BAM_FUNMAP: u16 = 4;
pub const BAM_FMUNMAP: u16 = 8;
pub const BAM_FREVERSE: u16 = 16;
pub const BAM_FMREVERSE: u16 = 32;
pub const BAM_FREAD1: u16 = 64;
pub const BAM_FREAD2: u16 = 128;
pub const BAM_FSECONDARY: u16 = 256;
pub const BAM_FQCFAIL: u16 = 512;
pub const BAM_FDUP: u16 = 1024;
pub const BAM_FSUPPLEMENTARY: u16 = 2048;


#[repr(C)]
pub struct sam_hdr_t { _unused: [u8; 0], }

impl sam_hdr_t {
	pub fn write<H: AsMut<htsFile>>(&self, mut hts_file: H) -> io::Result<()> {
		match unsafe { sam_hdr_write(hts_file.as_mut(), self) } {
			0 => Ok(()),
			_ => Err(hts_err("Failed to write SAM header".to_string())),
		}
	}
	pub fn nref(&self) -> usize { 
		let l = unsafe { sam_hdr_nref(self) };
		l as usize
	}
	fn check_idx(&self, i: usize) { if i >= self.nref() { panic!("Reference ID {} out of range", i); }}
	pub fn tid2name(&self, i: usize) -> &str {
		self.check_idx(i);
		from_cstr(unsafe { sam_hdr_tid2name(self, i as c_int) })
	}
	pub fn tid2len(&self, i: usize) -> usize {
		self.check_idx(i);
		let len = unsafe { sam_hdr_tid2len(self, i as c_int) };
		len as usize
	}
	pub fn name2tid<S: AsRef<str>>(&self, cname: S) -> Option<usize> {
		let tid = unsafe{ sam_hdr_name2tid(self, get_cstr(cname).as_ptr())};
		if tid < 0 { None } else { Some(tid as usize) }
	}
	pub fn text(&self) -> &str {
		from_cstr(unsafe { sam_hdr_str(self)})
	}
	pub fn dup(&self) -> io::Result<SamHeader> { 
		match NonNull::new(unsafe { sam_hdr_dup(self)}) {
			None => Err(hts_err("Failed to duplicate SAM/BAM header".to_string())),
			Some(p) => Ok(SamHeader{inner: p, phantom: PhantomData})
		}
	}
	pub fn add_lines<S: AsRef<str>>(&mut self, lines: S) -> io::Result<()> {
		let lines = lines.as_ref();
		match unsafe { sam_hdr_add_lines(self, get_cstr(lines).as_ptr(), lines.len() as size_t)} {
			0 => Ok(()),
			_ => Err(hts_err("Failed to add line to SAM/BAM header".to_string())),
		}
	}
	pub fn remove_except(&mut self, ln_type: &str, id_key: Option<&str>, id_value: Option<&str>) -> io::Result<()> {
		match if let (Some(key), Some(value)) = (id_key, id_value) {
			unsafe { sam_hdr_remove_except(self, get_cstr(ln_type).as_ptr(), get_cstr(key).as_ptr(), get_cstr(value).as_ptr()) }
		} else {
			unsafe { sam_hdr_remove_except(self, get_cstr(ln_type).as_ptr(), ptr::null(), ptr::null()) }
		} {
			0 => Ok(()),
			_ => Err(hts_err(format!("Failed to remove {} lines from SAM/BAM header", ln_type))),			
		}
	}
	pub fn remove(&mut self, ln_type: &str) -> io::Result<()> { self.remove_except(ln_type, None, None) }
}


#[repr(C)]
struct bam1_core_t {
	pos: HtsPos,
	tid: i32,
	bin: u16,
	qual: u8,
	l_extranul: u8,
	flag: u16,
	l_qname: u16,
	n_cigar: u32,
	l_qseq: i32,
	mtid: i32,
	mpos: HtsPos,
	isze: HtsPos,
}

#[repr(C)] 
pub struct bam1_t {
	core: bam1_core_t,
	id: u64,
	data: MaybeUninit<*mut c_char>,
	l_data: c_int,
	m_data: u32,
	mempolicy: u32,
}

impl bam1_t {
	fn data(&self) -> *const c_char {
		if self.data.as_ptr().is_null() { panic!("Attempt to read empty bam1_t structure") }
		else { unsafe {self.data.assume_init() }}
	}
	pub fn qname(&self) -> &str { from_cstr(self.data()) }
	pub fn endpos(&self) -> usize { unsafe{ bam_endpos(self) as usize} }
	pub fn tid(&self) -> Option<usize> { check_tid(self.core.tid) }
	pub fn mtid(&self) -> Option<usize> { check_tid(self.core.mtid) }
	pub fn qual(&self) -> u8 { self.core.qual }
	pub fn flag(&self) -> u16 { self.core.flag }
	pub fn pos(&self) -> Option<usize> { 
		if self.core.pos >=0 { Some(self.core.pos as usize) } else { None} 
	}
	pub fn mpos(&self) -> Option<usize> { 
		if self.core.mpos >=0 { Some(self.core.mpos as usize) } else { None} 
	}
	pub fn template_len(&self) -> isize { self.core.isze as isize }
	pub fn l_qseq(&self) -> i32 { self.core.l_qseq }
	pub fn qnames_eq(&self, b: &BamRec) -> bool {
		let i = unsafe { libc::strcmp(self.data(), b.data()) };
		i == 0
	}
	pub fn get_seq(&self) -> Option<&[u8]> {
		unsafe {
			let core = &self.core;
			let off = ((core.n_cigar as isize) << 2) + (core.l_qname as isize) as isize;
			let p = self.data().offset(off) as *const u8;
			if p.is_null() { None }
			else {
				let size = (core.l_qseq + 1) >> 1;
				Some(std::slice::from_raw_parts(p, size as usize))
			} 		
		}
	}
	pub fn get_qual(&self) -> Option<&[u8]> {
		unsafe {
			let core = &self.core;
			let off = ((core.n_cigar as isize) << 2) + (core.l_qname as isize) + ((core.l_qseq + 1) >> 1) as isize;
			let p = self.data().offset(off) as *const u8;
			if p.is_null() { None }
			else {
				let size = core.l_qseq;
				Some(std::slice::from_raw_parts(p, size as usize))
			} 		
		}
	}
	pub fn cigar(&self) -> Option<Cigar> {
		let len = self.core.n_cigar as usize;
		if len > 0 {
			let data = self.data();
			let slice = unsafe{ 
				let ptr: *const CigarElem = data.offset(self.core.l_qname as isize).cast();
				std::slice::from_raw_parts(ptr, len) 
			};
			Some(Cigar(slice))
		} else { None }
	}
	pub fn qlen(&self) -> Option<u32> { self.cigar().map(|c| c.qlen() )}
	pub fn rlen(&self) -> Option<u32> { self.cigar().map(|c| c.rlen() )}
	pub fn cigar_buf(&self) -> Option<CigarBuf> { self.cigar().map(|c| c.to_cigar_buf()) }
	pub fn write<H: AsMut<htsFile>, S: AsMut<sam_hdr_t>>(&mut self, mut hfile: H, mut hdr: S) -> io::Result<usize> {
		match unsafe { sam_write1(hfile.as_mut(), hdr.as_mut(), self) } {
			x if x >= 0 => Ok(x as usize),
			_ => Err(hts_err("Failed to write BamRec".to_string())),
		}	
	}
	pub fn aux_update_str<S: AsRef<str>>(&mut self, tag: &str, data: S) -> io::Result<()> {
		if tag.len() != 2 { return Err(hts_err("Failed to update string tag: tag length is not 2".to_string())) }
		let data = data.as_ref();
		match unsafe { bam_aux_update_str(self, get_cstr(tag).as_ptr(), data.len() as c_int, get_cstr(data).as_ptr())} {
			0 => Ok(()),
			_ => Err(hts_err("Failed to update string tag".to_string())),
		}
	}
	pub fn get_aux(&self) -> Option<&[u8]> {
		unsafe {
			let core = &self.core;
			let off = ((core.n_cigar as isize) << 2) + (core.l_qname as isize) + (core.l_qseq + ((core.l_qseq + 1) >> 1)) as isize;
			let p = self.data().offset(off) as *mut u8;
			if p.is_null() { None }
			else {
				let size = self.l_data as isize - off;
				if size < 0 { panic!("Invalid BAM aux size") }
				Some(std::slice::from_raw_parts(p, size as usize))
			} 
		}
	}
	pub fn get_aux_iter(&self) -> Option<BamAuxIter> { 
		if let Some(aux) = self.get_aux() {	Some(BamAuxIter{data: aux}) } else { None }
	}
	pub fn get_tag(&self, tag_id: &str, tag_type: char) -> Option<&[u8]> {
		if tag_id.len() != 2 { return None } 
		let tag_id = tag_id.as_bytes();
		if let Some(itr) = self.get_aux_iter() {
			for tag in itr {
				if tag[0] == tag_id[0] && tag[1] == tag_id[1] && tag[2] == (tag_type as u8) { return Some(&tag[3..]) }
			}
		}
		None
	}
	pub fn get_seq_qual(&self) -> io::Result<SeqQual> {
		let seq = self.get_seq().ok_or_else(|| hts_err("No Sequence data in BAM record".to_string()))?;
		let qual = self.get_qual().ok_or_else(|| hts_err("No Quality data in BAM record".to_string()))?;
		let mut sq = Vec::with_capacity(qual.len());
		let mut qitr = qual.iter();
		for s in seq.iter() {
			let (b, a) = SEQ_DECODE[*s as usize];
			let q = (*qitr.next().unwrap()).min(62);
			sq.push(if a > 0 { (a - 1) | (q << 2) } else { 0 });
			if let Some(x) = qitr.next() {
				let q = (*x).min(62);
				sq.push(if b > 0 { (b - 1) | (q << 2) } else { 0 });
			}
		}
		Ok(SeqQual(sq.into_boxed_slice()))
	}
	
	pub fn read<H: AsMut<htsFile>, S: AsMut<sam_hdr_t>>(&mut self, mut hfile: H, mut hdr: S) -> SamReadResult {
		match unsafe { sam_read1(hfile.as_mut(), hdr.as_mut(), self) } {
			0..=c_int::MAX => SamReadResult::Ok,
			-1 => SamReadResult::EOF,
			_ => SamReadResult::Error,
		}
	}
}

#[link(name = "hts")]
extern "C" {
	fn sam_hdr_read(fp_: *mut htsFile) -> *mut sam_hdr_t;
	fn sam_hdr_write(fp_: *mut htsFile, hd_: *const sam_hdr_t) -> c_int;
	fn sam_hdr_destroy(hd_: *mut sam_hdr_t);
	fn sam_hdr_dup(hd_: *const sam_hdr_t) -> *mut sam_hdr_t;
	fn sam_hdr_add_lines(hd_: *mut sam_hdr_t, lines_: *const c_char, len_: size_t) -> c_int;
	fn sam_hdr_remove_except(hd_: *mut sam_hdr_t, type_: *const c_char, id_key_: *const c_char, id_value_: *const c_char) -> c_int;
	fn sam_hdr_nref(hd_: *const sam_hdr_t) -> c_int;
	fn sam_hdr_tid2name(hd_: *const sam_hdr_t, i_: c_int) -> *const c_char;
	fn sam_hdr_tid2len(hd_: *const sam_hdr_t, i_: c_int) -> c_int;
	fn sam_hdr_name2tid(hd_: *const sam_hdr_t, nm_: *const c_char) -> c_int;
	fn sam_hdr_str(hd_: *const sam_hdr_t) -> *const c_char;
	fn bam_init1() -> *mut bam1_t;
	fn bam_destroy1(b: *mut bam1_t);
	fn bam_endpos(pt_: *const bam1_t) -> HtsPos;
	fn bam_aux_update_str(pt_: *mut bam1_t, tag_: *const c_char, len_: c_int, data_: *const c_char) -> c_int;
	fn sam_read1(fp_: *mut htsFile, hd_: *mut sam_hdr_t, b_: *mut bam1_t) -> c_int;
	fn sam_write1(fp_: *mut htsFile, hd_: *mut sam_hdr_t, b_: *const bam1_t) -> c_int;
}

pub struct SamHeader {
	inner: NonNull<sam_hdr_t>,
	phantom: PhantomData<sam_hdr_t>,	
}

unsafe impl Sync for SamHeader{}
unsafe impl Send for SamHeader{}

impl <'a>Drop for SamHeader {
	fn drop(&mut self) {
		unsafe { sam_hdr_destroy(self.as_mut()) };
	}
}

impl Deref for SamHeader {
	type Target = sam_hdr_t;
	#[inline]
	fn deref(&self) -> &sam_hdr_t { unsafe{self.inner.as_ref()} }	
}

impl DerefMut for SamHeader {
	#[inline]
	fn deref_mut(&mut self) -> &mut sam_hdr_t {unsafe{ self.inner.as_mut() }}
}

impl AsRef<sam_hdr_t> for SamHeader {
	#[inline]
	fn as_ref(&self) -> &sam_hdr_t { self}	
}

impl AsMut<sam_hdr_t> for SamHeader {
	#[inline]
	fn as_mut(&mut self) -> &mut sam_hdr_t { self}	
}

impl SamHeader {
	pub fn read<H: AsMut<htsFile>>(mut hts_file: H) -> io::Result<Self> {
		match NonNull::new(unsafe { sam_hdr_read(hts_file.as_mut()) }) {
			None => Err(hts_err("Failed to load SAM header".to_string())),
			Some(p) => Ok(Self{inner: p, phantom: PhantomData})
		}
	}
}

// Note that Trim is non-standard - we use it internally
#[repr(u8)]
#[derive(PartialEq,Eq)]
pub enum CigarOp {
	Match, Ins, Del, RefSkip, SoftClip, HardClip, Pad, Equal, Diff, Back, Overlap, Invalid1, Invalid2, Invalid3, Invalid4, Invalid5
}

impl fmt::Display for CigarOp {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", match self {
			CigarOp::Match => 'M',
			CigarOp::Ins => 'I',
			CigarOp::Del => 'D',
			CigarOp::RefSkip => 'N',
			CigarOp::SoftClip => 'S',
			CigarOp::HardClip => 'H',
			CigarOp::Pad => 'P',
			CigarOp::Equal => '=',
			CigarOp::Diff => 'X',
			CigarOp::Back => 'D',
			CigarOp::Overlap => 'O',
			_ => '?',
		})
	}
}

#[derive(Copy, Clone)]
pub struct CigarElem(u32);
const CIGAR_TYPE: u32 = 0x13C1A7;
const CIGAR_TYPE1: u32 = 0x13C5A7;

impl CigarElem {
	pub fn op_len(&self) -> u32 { self.0 >> 4 }
	pub fn op(&self) -> CigarOp { unsafe { std::mem::transmute((self.0 & 15) as u8)}}
	pub fn op_pair(&self) -> (CigarOp, u32) {(self.op(), self.op_len())}

	// This magic comes from htslib/sam.h
	// If bit 0 is set in op_type then the op consumes the query, and
	// if bit 1 is set then the op consumes the reference
	pub fn op_type(&self) -> u32 { (CIGAR_TYPE >> ((self.0 & 15) << 1)) & 3 }
	// Similar to above, but we also count Hard clips the same as Soft clips
	pub fn op_type1(&self) -> u32 { (CIGAR_TYPE1 >> ((self.0 & 15) << 1)) & 3 }
}

const CIGAR_STR:&str = "MIDNSHP=XDO?????";

impl fmt::Display for CigarElem {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}{}", self.0 >> 4, CIGAR_STR.as_bytes()[(self.0 & 15) as usize]as char)
	}
}

pub struct Cigar<'a>(&'a[CigarElem]);

impl <'a> Deref for Cigar<'a> {
    type Target = [CigarElem];
	fn deref(&self) -> &[CigarElem] { &self.0 }
}

impl <'a> Cigar<'a> {
	pub fn qlen(&self) -> u32 {	self.iter().filter(|c| (c.op_type() & 1) != 0).fold(0, |mut l, c| {l += c.op_len(); l}) }
	pub fn qlen1(&self) -> u32 { self.iter().filter(|c| (c.op_type1() & 1) != 0).fold(0, |mut l, c| {l += c.op_len(); l}) }
	pub fn rlen(&self) -> u32 {	self.iter().filter(|c| (c.op_type() & 2) != 0).fold(0, |mut l, c| {l += c.op_len(); l}) }
	pub fn to_cigar_buf(&self) -> CigarBuf {
		let v = self.to_vec();
		CigarBuf(v.into_boxed_slice())
	}
}

pub struct CigarBuf(Box<[CigarElem]>);

impl Deref for CigarBuf {
    type Target = [CigarElem];
	fn deref(&self) -> &[CigarElem] { self.0.deref() }
}

impl fmt::Display for CigarBuf {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		for elem in self.iter() { write!(f, "{}", elem)? }
		Ok(())
	}
}

const BAM_COVERLAP:u32 = 10;

fn trim_cigar_vec<I: Iterator<Item = CigarElem>>(it: I, x: u32) -> Vec<CigarElem> {
	let mut ct = 0;
	let mut v = Vec::new();
	for elem in it {
		if ct >= x || (elem.op_type() & 2) == 0 { v.push(elem)}
		else {
			let l = elem.op_len();
			if (elem.op_type() & 1) != 0 { 	
				if ct + l <= x {
					v.push(CigarElem((elem.0 & 0xfffffff0) | BAM_COVERLAP));
				} else {
					v.push(CigarElem(((x - ct) << 4) | BAM_COVERLAP));
					v.push(CigarElem(((ct + l - x) << 4) | (elem.0 & 15)));
				}
			}
			ct += l;	
		}
	}
	v
}

impl CigarBuf {
	pub fn qlen(&self) -> u32 {	self.iter().filter(|c| (c.op_type() & 1) != 0).fold(0, |mut l, c| {l += c.op_len(); l}) }
	pub fn qlen1(&self) -> u32 { self.iter().filter(|c| (c.op_type1() & 1) != 0).fold(0, |mut l, c| {l += c.op_len(); l}) }
	pub fn rlen(&self) -> u32 {	self.iter().filter(|c| (c.op_type() & 2) != 0).fold(0, |mut l, c| {l += c.op_len(); l}) }

	// Adjust cigar so that alignment starts x bases later w.r.t the reference
	pub fn trim_start(&mut self, x: u32) {
		let v = trim_cigar_vec(self.iter().copied(), x);
		self.0 = v.into_boxed_slice();		
	}
	// Adjust cigar so that alignment ends x bases earlier w.r.t the reference
	pub fn trim_end(&mut self, x: u32) {
		let mut v = trim_cigar_vec(self.iter().copied().rev(), x);
		let v1: Vec<_> = v.drain(..).rev().collect();
		self.0 = v1.into_boxed_slice();
	}
}

const BAM_CIGAR_TAB: [i8; 256] = [
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,  7, -1, -1,
	-1, -1,  9, -1,  2, -1, -1, -1,  5,  1, -1, -1, -1,  0,  3, -1,
	 6, -1, -1,  4, -1, -1, -1, -1,  8, -1, -1, -1, -1, -1, -1, -1,
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,	
];

const BAM_CIGAR_MAX_LEN: u32 = 1<<28;

impl FromStr for CigarBuf {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut sp = s;
		let mut v = Vec::new();
		while !sp.is_empty() {
			let i = sp.find(|c: char| !c.is_ascii_digit()).ok_or("Cigar string does not end in letter")?;
			let n = <u32>::from_str(&sp[0..i]).map_err(|_| "Error parsing Cigar string - expecting number")?;
			if n >= BAM_CIGAR_MAX_LEN { return Err("Cigar number too large")};
			let op = BAM_CIGAR_TAB[sp[i..=i].as_bytes()[0] as usize];
			if op < 0 { return Err("Illegal Cigar character") }
			v.push(CigarElem((n << 4) | (op as u32)));
			sp = &sp[i+1..];
		}
		Ok(CigarBuf(v.into_boxed_slice()))		
    }
}

pub struct BamRec {	
	inner: NonNull<bam1_t>,
	phantom: PhantomData<bam1_t>,
}

impl Deref for BamRec {
	type Target = bam1_t;
	#[inline]
	fn deref(&self) -> &bam1_t { unsafe{self.inner.as_ref()} }	
}

impl DerefMut for BamRec {
	#[inline]
	fn deref_mut(&mut self) -> &mut bam1_t {unsafe{ self.inner.as_mut() }}
}

impl AsRef<bam1_t> for BamRec {
	#[inline]
	fn as_ref(&self) -> &bam1_t { self}	
}

impl AsMut<bam1_t> for BamRec {
	#[inline]
	fn as_mut(&mut self) -> &mut bam1_t { self}	
}

impl Drop for BamRec {
	fn drop(&mut self) {
		unsafe { bam_destroy1(self.as_mut())}
	}
}

const SEQ_DECODE:[(u8, u8); 256] = [
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,1), (1,1), (2,1), (0,1), (3,1), (0,1), (0,1), (0,1), (4,1), (0,1), (0,1), (0,1), (0,1), (0,1), (0,1), (0,1),
	(0,2), (1,2), (2,2), (0,2), (3,2), (0,2), (0,2), (0,2), (4,2), (0,2), (0,2), (0,2), (0,2), (0,2), (0,2), (0,2),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,3), (1,3), (2,3), (0,3), (3,3), (0,3), (0,3), (0,3), (4,3), (0,3), (0,3), (0,3), (0,3), (0,3), (0,3), (0,3),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,4), (1,4), (2,4), (0,4), (3,4), (0,4), (0,4), (0,4), (4,4), (0,4), (0,4), (0,4), (0,4), (0,4), (0,4), (0,4),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0),
	(0,0), (1,0), (2,0), (0,0), (3,0), (0,0), (0,0), (0,0), (4,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0), (0,0)
];

fn check_tid(i: c_int) -> Option<usize> {
	if i >= 0 { Some( i as usize) }
	else { None }		
}

impl BamRec {
	// bam_init1() simply zeroes the newly allocated structure, so the data ptr is set to zero
	// we mark this as Maybe Uninit so we have to check that it is non-null be dereferencing
	pub fn new() -> io::Result<Self> { 
		match NonNull::new(unsafe{bam_init1()}) {
			Some(mut b) => {
				let brec = unsafe{ b.as_mut() };
				brec.data = MaybeUninit::<*mut c_char>::zeroed();
				Ok(BamRec{inner: b, phantom: PhantomData}) 
			},
			None => Err(hts_err("Failed to allocate new BamRec".to_string())),
		}
	}
	pub fn swap(&mut self, other: &mut Self) {
		let t = self.inner;
		self.inner = other.inner;
		other.inner = t;
	}
}

fn aux_type2size(tp: u8) -> u8 {
	match tp {
		b'A' | b'c' | b'C' => 1,
		b's' | b'S' => 2,
		b'i' | b'I' | b'f' => 4,
		b'd' => 8,
		b'Z' | b'H' | b'B' => tp,
		_ => 0,
	}
}

pub struct BamAuxIter<'a> { data: &'a[u8] }

impl <'a>Iterator for BamAuxIter<'a> {
	type Item = &'a[u8];
	fn next(&mut self) -> Option<Self::Item> {
		let ln = self.data.len();
		if ln < 3 { None }
		else {
			let mut l = 3;
			match aux_type2size(self.data[2]) {
				b'Z' | b'H' => {
					while l < ln && self.data[l] != 0 { l += 1 }
					if l < ln { l += 1 }
				},
				b'B' => {
					if ln - l < 5 {return None}
					let sz = aux_type2size(self.data[l]) as usize;
					let n = u32::from_le_bytes(self.data[l+1..l+5].try_into().unwrap()) as usize;
					l += 5;
					if sz == 0 { return None }
					l += sz * n;
				},
				0 => return None,
				sz => l += sz as usize,
			}
			if l > ln { None }
			else { 
				let (a, b) = self.data.split_at(l);
				self.data = b;
				Some(a)
			}
		}	
	}	
}

pub enum SamReadResult {
	Ok,
	EOF,
	Error,
}

pub struct SeqQual(Box<[u8]>);

impl Deref for SeqQual {
    type Target = [u8];
	fn deref(&self) -> &[u8] { self.0.deref() }
}

const FMT_BASES: [char;256] = [
	'N', 'N', 'N', 'N', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
	'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 'A', 'C', 'G', 'T', 
];

const FMT_QUAL: [u8;256] = [
	33,33,33,33,34,34,34,34,35,35,35,35,36,36,36,36,
	37,37,37,37,38,38,38,38,39,39,39,39,40,40,40,40,
	41,41,41,41,42,42,42,42,43,43,43,43,44,44,44,44,
	45,45,45,45,46,46,46,46,47,47,47,47,48,48,48,48,
	49,49,49,49,50,50,50,50,51,51,51,51,52,52,52,52,
	53,53,53,53,54,54,54,54,55,55,55,55,56,56,56,56,
	57,57,57,57,58,58,58,58,59,59,59,59,60,60,60,60,
	61,61,61,61,62,62,62,62,63,63,63,63,64,64,64,64,
	65,65,65,65,66,66,66,66,67,67,67,67,68,68,68,68,
	69,69,69,69,70,70,70,70,71,71,71,71,72,72,72,72,
	73,73,73,73,74,74,74,74,75,75,75,75,76,76,76,76,
	77,77,77,77,78,78,78,78,79,79,79,79,80,80,80,80,
	81,81,81,81,82,82,82,82,83,83,83,83,84,84,84,84,
	85,85,85,85,86,86,86,86,87,87,87,87,88,88,88,88,
	89,89,89,89,90,90,90,90,91,91,91,91,92,92,92,92,
	93,93,93,93,94,94,94,94,95,95,95,95,96,96,96,96,
];

impl fmt::Display for SeqQual {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut s = String::with_capacity(self.len());
		if f.alternate() {
			for sq in self.iter() { s.push(FMT_QUAL[*sq as usize] as char)}
		} else {
			for sq in self.iter() {	s.push(FMT_BASES[*sq as usize])}
		}
		f.write_str(&s)
	}
}


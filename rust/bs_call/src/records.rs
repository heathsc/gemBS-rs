use std::str::{FromStr, from_utf8};
use super::htslib::*;
use super::config::ConfHash;

#[derive(Copy,Clone,PartialEq,Eq)]
struct MapPos {
	pos: u32,
	tid: u32,
}

const MFLAG_REVERSE:u16 = 4;
const MFLAG_SPLIT:u16 = 8;
const MFLAG_LAST: u16 = 16;
const MFLAG_DUPLICATE: u16 = 32;
const MFLAG_BSSTRAND_MASK:u16 = 3;

struct Map {
	map_pos: MapPos,
	cigar: CigarBuf,
	mapq: u8,
	split_id: u8,
	flags: u16,
}

const BSSTRAND_MAP:[BSStrand; 4] = [BSStrand::Unknown, BSStrand::StrandC2T, BSStrand::StrandG2A, BSStrand::Unconverted];


impl Map {
	pub fn bs_strand(&self) -> BSStrand { BSSTRAND_MAP[(&self.flags & MFLAG_BSSTRAND_MASK) as usize] }
	pub fn flags(&self) -> u16 { self.flags }
	pub fn from_bam_rec(sam_hdr: &SamHeader, brec: &BamRec) -> Option<Vec<Self>> {
		
		// Handle basic fields
		let flag = brec.flag();
		let tst_flag = |x| (flag & x) != 0;
		if tst_flag(BAM_FUNMAP) { return None }
		let (bs_strand, sa_tag) = get_bs_strand(brec);
		let mut flags = bs_strand.get_num();
		if tst_flag(BAM_FDUP) { flags |= MFLAG_DUPLICATE }; 
		if tst_flag(BAM_FREVERSE) { flags |= MFLAG_REVERSE }; 
		let tid = if let Some(i) = brec.tid() {i as u32} else {return None};
		let pos = if let Some(i) = brec.pos() {i as u32} else {return None};
		let map_pos = MapPos{tid, pos};
		let mut maps = vec!(Map {map_pos, mapq: brec.qual(), split_id: 0, cigar: brec.cigar().expect("No CIGAR field").to_cigar_buf(), flags});
		// Handle SA tag
		if let Some(tag) = sa_tag.map(|s| from_utf8(s).unwrap() ) {
			for sa_map in tag.split(';') {
				if let Some(m) = Map::from_sa_tag(sam_hdr, sa_map) { maps.push(m) }
			}
		}
		// Get order in read for split reads
		if maps.len() > 1 {
			if maps.len() > 256 { panic!("Too many splits in read id {} (max = 256)", brec.qname()) }
			let mut map_idx: Vec<_> = maps.iter().enumerate().map(|(i,_)| i).collect();
			// Sort by starting position of mappng (i.e. after discarding first hard or soft clip)
			map_idx.sort_by_cached_key(|ix| {
				let cig = &maps[*ix].cigar;
				match cig[0].op() {
					CigarOp::SoftClip | CigarOp::HardClip => cig[0].op_len(),
					_ => 0,
				}
			});
			// If current read is reversed then the split order also needs to be reversed
			let idx = if tst_flag(BAM_FREVERSE) {
				let l = map_idx.len();
				for idx in map_idx.iter() {
					maps[*idx].flags |= MFLAG_SPLIT;
					maps[*idx].split_id = (l - 1 - *idx) as u8;
				}
				map_idx[0]
			} else {
				for idx in map_idx.iter() {
					maps[*idx].flags |= MFLAG_SPLIT;
					maps[*idx].split_id = *idx as u8;
				}
				*(map_idx.last().unwrap())
			};
			maps[idx].flags |= MFLAG_LAST;
		} else { maps[0].flags |= MFLAG_LAST }
		Some(maps)
	}
	pub fn from_sa_tag(sam_hdr: &SamHeader, tag: &str) -> Option<Self> {
		let v: Vec<_> = tag.trim().split(',').collect();
		if !v.is_empty() {
			if v.len() != 6 { panic!("Wrong number of fields in SA tag (expected 6, found {})", v.len())}
			let tid = if let Some(t) = sam_hdr.name2tid(v[0]) {t as u32} else {panic!("Unknown contig {} in SA tag {}", v[0], tag)};
			let pos = if let Ok(p) = <u32>::from_str(v[1]) {p} else {panic!("Couldn't parse position {} in SA tag {}", v[1], tag)};
			let flags = match v[2] {
				"+" => MFLAG_SPLIT,
				"-" => MFLAG_SPLIT | MFLAG_REVERSE,
				_ => panic!("Couldn't parse strand information {} in SA tag {}", v[2], tag),
			};
			let cigar = match CigarBuf::from_str(v[3]) {
				Ok(c) => c,
				Err(e) => panic!("Couldn't parse Cigar {} in SA tag {}: {}", v[3], tag, e),
			};
			let mapq = if let Ok(x) = <u8>::from_str(v[4]) {x} else {panic!("Couldn't parse mapq {} in SA tag {}", v[4], tag)};
			Some(Map{map_pos: MapPos{pos, tid}, flags, cigar, mapq, split_id: 0})
		} else { None }
	}
}

struct ReadEnd {
	id: String,
	// For split mapped reads we get the details of all of the splits from the SA tag
	// The current mapping is always the first element in maps
	maps: Vec<Map>, 
	seq_qual: SeqQual,
	// For paired reads
	mate_pos: Option<MapPos>,
}

#[derive(Copy,Clone,PartialEq,Eq)]
pub enum ReadFlag {
	Unmapped,
	Secondary,
	Supplementary,
	MateUnmapped,
	QCFail,
	Duplicate,
	NotAligned,
	NotCorrectlyPaired,
	BadOrientation,
	MismatchedContigs,
	LongInsertLength,
	LowMapQ,
	None,	
}

impl ReadEnd {
	pub fn from_bam_rec(conf_hash: &ConfHash, sam_hdr: &SamHeader, brec: &BamRec) -> (Option<Self>, ReadFlag) {
		// Pull in config options
		let keep_duplicates = conf_hash.get_bool("keep_duplicates");
		let keep_unmatched = conf_hash.get_bool("keep_unmatched");
		let keep_supplementary = conf_hash.get_bool("keep_supplementary");
		let ignore_duplicates = conf_hash.get_bool("ignore_duplicates");
		let maxq_threshold = conf_hash.get_int("maxq_threshold");
		let max_template_length = conf_hash.get_int("maxq_template_length");
		
		// Check Bam record
		let flag = brec.flag();
		let mut read_flag = ReadFlag::None;
		let tst_flag = |x| (flag & x) != 0;
		let mut set_read_flag = |x| if read_flag != ReadFlag::None { read_flag = x };
		// Filter out the simple cases
		if tst_flag(BAM_FUNMAP) { return (None, ReadFlag::Unmapped)};
		if tst_flag(BAM_FQCFAIL) { return (None, ReadFlag::QCFail)};
		if tst_flag(BAM_FSECONDARY) { return (None, ReadFlag::Secondary)};
		if tst_flag(BAM_FDUP) { 
			if keep_duplicates { set_read_flag(ReadFlag::Duplicate) } else { return (None, ReadFlag::Duplicate)}
		}
		if tst_flag(BAM_FSUPPLEMENTARY) {
			if keep_supplementary { set_read_flag(ReadFlag::Supplementary) }  else {return (None, ReadFlag::Supplementary)}
		}
		let mut map = Map::from_bam_rec(sam_hdr, brec).expect("Couldn't get map record from BAM read");
		
		if (brec.qual() as usize) < maxq_threshold { return (None, ReadFlag::LowMapQ)};
		
		let tid = brec.tid().unwrap_or_else(|| panic!("Error in BAM: no contig id for mapped read (id: {})", brec.qname()));
		let multi_seg = if tst_flag(BAM_FPAIRED) {
			if tst_flag(BAM_FMUNMAP) { 
				if keep_unmatched { set_read_flag(ReadFlag::MateUnmapped)} else {return (None, ReadFlag::MateUnmapped)}
				false
			} else {
				let mut mflag = true;
				let mtid = brec.mtid().unwrap_or_else(|| panic!("Error in BAM: no mate contig id for mapped mate (id: {})", brec.qname()));
				if tid != mtid {
					mflag = false;
					if keep_unmatched { set_read_flag(ReadFlag::MismatchedContigs)} else {return (None, ReadFlag::MismatchedContigs)}
				}
				if mflag {
					// Check if reads are on the same strand
					
					let f = flag & (BAM_FREVERSE | BAM_FMREVERSE);
					if f == 0 || f == (BAM_FREVERSE | BAM_FMREVERSE) {
						
					}
				}
				mflag
			}
			
		} else { false };
		(None, read_flag)
	}
}

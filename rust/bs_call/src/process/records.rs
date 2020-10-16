
use std::str::{FromStr, from_utf8};
use crate::htslib::*;
use crate::config::ConfHash;
use crate::stats::stats_json::FSReadLevelType as ReadFlag;

#[derive(Copy,Clone,PartialEq,Eq)]
pub struct MapPos {
	pub pos: u32,
	pub tid: u32,
}

pub const MFLAG_REVERSE:u16 = 4;
pub const MFLAG_SPLIT:u16 = 8;
pub const MFLAG_LAST: u16 = 16;
pub const MFLAG_DUPLICATE: u16 = 32;
pub const MFLAG_READ1: u16 = 64;
pub const MFLAG_READ2: u16 = 128;
pub const MFLAG_BSSTRAND_MASK:u16 = 3;
pub const MFLAG_DUPLICATE_MASK: u16 = 3;

pub struct Map {
	pub map_pos: MapPos,
	pub cigar: CigarBuf,
	pub mapq: u8,
	pub split_id: u8,
	pub flags: u16,
}

impl PartialEq for Map {
    fn eq(&self, other: &Self) -> bool { self.map_pos == other.map_pos && ((self.flags ^ other.flags) & MFLAG_DUPLICATE_MASK) == 0 }
}

const BSSTRAND_MAP:[BSStrand; 4] = [BSStrand::Unknown, BSStrand::StrandC2T, BSStrand::StrandG2A, BSStrand::Unconverted];
pub const BSS_UNKNOWN: u16 = 0;
pub const BSS_C2T: u16 = 1;
pub const BSS_G2A: u16 = 2;
pub const BSS_UNCONVERTED: u16 = 3;

impl Map {
	pub fn bs_strand(&self) -> BSStrand { BSSTRAND_MAP[(self.flags & MFLAG_BSSTRAND_MASK) as usize] }
	pub fn is_last(&self) -> bool { (self.flags & MFLAG_LAST) != 0 }
	pub fn is_reverse(&self) -> bool { (self.flags & MFLAG_REVERSE) != 0 }
	pub fn start(&self) -> u32 { self.map_pos.pos }
	pub fn end(&self) -> u32 { self.map_pos.pos + self.cigar.rlen() - 1 }
	pub fn rlen(&self) -> u32 { self.cigar.rlen() }
}


fn maps_from_bam_rec(sam_hdr: &SamHeader, brec: &BamRec, check_supp: bool) -> Result<Vec<Map>, String> {
	// Handle basic fields
	let flag = brec.flag();
	let tst_flag = |x| (flag & x) != 0;
	if tst_flag(BAM_FUNMAP) { return Err("Read unmapped".to_string()) }
	let (bs_strand, sa_tag) = get_bs_strand(brec);
	let mut flags = bs_strand.get_num();
	if tst_flag(BAM_FDUP) { flags |= MFLAG_DUPLICATE }; 
	if tst_flag(BAM_FREVERSE) { flags |= MFLAG_REVERSE }; 
	if tst_flag(BAM_FREAD1) { flags |= MFLAG_READ1 }; 
	if tst_flag(BAM_FREAD2) { flags |= MFLAG_READ2 }; 
	if tst_flag(BAM_FSUPPLEMENTARY) { flags |= MFLAG_SPLIT };
	let mut maps = Vec::with_capacity(1);
	let tid = if let Some(i) = brec.tid() {i as u32} else {return Err("Invalid contig ID".to_string())};
	let pos = if let Some(i) = brec.pos() {i as u32} else {return Err("Invalid map position".to_string())};
	let map_pos = MapPos{tid, pos};
	let cigar = brec.cigar_buf().expect("No CIGAR field");
	maps.push(Map{map_pos, flags, mapq: brec.qual(), cigar, split_id: 0});
	// Handle SA tag
	if check_supp {
		if let Some(tag) = sa_tag.map(|s| from_utf8(s).unwrap() ) {
			for sa_map in tag.split(';') {
				if sa_map.trim().len() > 1 {
					maps.push(map_from_sa_tag(sam_hdr, sa_map)?);
				}
			}
		}
	}
	// Get order in read for split reads and set flags
	// Note we assume that all maps are from the same BS strand unless the strand information
	// is supplied in the SA tag
	if maps.len() > 1 {
		if maps.len() > 256 { panic!("Too many splits in read id {} (max = 256)", brec.qname()) }
		let mut map_idx: Vec<_> = maps.iter().enumerate().map(|(i,_)| i).collect();
		// Sort by starting position of mapping (i.e. after discarding first hard or soft clip)
		map_idx.sort_by_cached_key(|ix| {
			let cig = &maps[*ix].cigar;
			let cig_elem = if (maps[*ix].flags & MFLAG_REVERSE) == 0 { &cig[0] } else { &cig.last().unwrap() }; 
			match cig_elem.op() {
				CigarOp::SoftClip | CigarOp::HardClip => cig_elem.op_len(),
				_ => 0,
			}
		});
		for idx in map_idx.iter() {
			maps[*idx].flags |= if maps[*idx].bs_strand() == BSStrand::Unknown { bs_strand.get_num() | MFLAG_SPLIT }
			else { MFLAG_SPLIT };
			maps[*idx].split_id = *idx as u8;
		}
		let idx = *(map_idx.last().unwrap());
		maps[idx].flags |= MFLAG_LAST;
	} else { maps[0].flags |= MFLAG_LAST }
	Ok(maps)
}

fn map_from_sa_tag(sam_hdr: &SamHeader, tag: &str) -> Result<Map, String> {
		let v: Vec<_> = tag.trim().split(',').collect();
		if !v.is_empty() {
		if v.len() != 6 { return Err(format!("Wrong number of fields in SA tag (expected 6, found {})", v.len())) }
		let tid = sam_hdr.name2tid(v[0]).ok_or_else(|| format!("Unknown contig {} in SA tag {}", v[0], tag))? as u32;
		let pos = <u32>::from_str(v[1]).map_err(|e| format!("Couldn't parse position {} in SA tag {}: {}", v[1], tag, e))?;
		let flags = match v[2] {
			"+" => MFLAG_SPLIT,
			"+C" => MFLAG_SPLIT | BSS_C2T,
			"+G" => MFLAG_SPLIT | BSS_G2A,
			"-" => MFLAG_SPLIT | MFLAG_REVERSE,
			"-C" => MFLAG_SPLIT | MFLAG_REVERSE | BSS_C2T,
			"-G" => MFLAG_SPLIT | MFLAG_REVERSE | BSS_G2A,
			_ => return Err(format!("Couldn't parse strand information {} in SA tag {}", v[2], tag)),
		};
		let cigar = match CigarBuf::from_str(v[3]) {
			Ok(c) => c,
			Err(e) => panic!("Couldn't parse Cigar {} in SA tag {}: {}", v[3], tag, e),
		};
		let mapq = if let Ok(x) = <u8>::from_str(v[4]) {x} else {panic!("Couldn't parse mapq {} in SA tag {}", v[4], tag)};
		Ok(Map{map_pos: MapPos{pos, tid}, flags, cigar, mapq, split_id: 0})
	} else { Err("Empty SA tag".to_string()) }
}

pub struct ReadEnd {
	// For split mapped reads we get the details of all of the splits from the SA tag
	// The current mapping is always the first element in maps
	pub maps: Vec<Map>, 
	pub seq_qual: SeqQual,
	// For paired reads
	pub mate_pos: Option<MapPos>,
	pub mate_idx: Option<usize>,
}

impl ReadEnd {
	pub fn from_bam_rec(conf_hash: &ConfHash, sam_hdr: &SamHeader, brec: &BamRec) -> (Option<Self>, ReadFlag) {
		// Pull in config options
		let keep_duplicates = conf_hash.get_bool("keep_duplicates");
		let keep_unmatched = conf_hash.get_bool("keep_unmatched");
		let keep_supplementary = conf_hash.get_bool("keep_supplementary");
		let ignore_duplicates = conf_hash.get_bool("ignore_duplicates");
		let mapq_threshold = conf_hash.get_int("mapq_threshold");
		let max_template_length = conf_hash.get_int("max_template_length");
		
		// Check Bam record
		let flag = brec.flag();
		let mut read_flag = ReadFlag::Passed;
		let tst_flag = |x| (flag & x) != 0;
		let mut set_read_flag = |x| if read_flag == ReadFlag::Passed { read_flag = x };
		// Filter out the simple cases
		if tst_flag(BAM_FUNMAP) { return (None, ReadFlag::Unmapped)};
		if tst_flag(BAM_FQCFAIL) { return (None, ReadFlag::QCFlags)};
		if tst_flag(BAM_FSECONDARY) { return (None, ReadFlag::SecondaryAlignment)};
		if !ignore_duplicates && tst_flag(BAM_FDUP) { 
			if keep_duplicates { set_read_flag(ReadFlag::Duplicate) } else { return (None, ReadFlag::Duplicate)}
		}
		if tst_flag(BAM_FSUPPLEMENTARY) {
			if keep_supplementary { set_read_flag(ReadFlag::SupplementaryAlignment) }  else {return (None, ReadFlag::SupplementaryAlignment)}
		}
		if (brec.qual() as usize) < mapq_threshold { return (None, ReadFlag::LowMAPQ)};
		let tid = if let Some(x) = brec.tid() {x} else { return (None, ReadFlag::NoPosition) };
		if brec.pos().is_none() { return (None, ReadFlag::NoPosition) };
		let mate_pos = if tst_flag(BAM_FPAIRED) {
			if tst_flag(BAM_FMUNMAP) { 
				if keep_unmatched { set_read_flag(ReadFlag::MateUnmapped)} else {return (None, ReadFlag::MateUnmapped)}
				None
			} else {
				let mtid = if let Some(x) = brec.mtid() {x} else { return (None, ReadFlag::NoMatePosition) };
				let mpos = if let Some(x) = brec.mpos() {x} else { return (None, ReadFlag::NoMatePosition) };
				if mtid != tid {
					if keep_unmatched { set_read_flag(ReadFlag::MisMatchContig)} else {return (None, ReadFlag::MisMatchContig)}				
				}
				if brec.template_len().abs() > max_template_length as isize {
					if keep_unmatched { set_read_flag(ReadFlag::LargeInsertSize)} else {return (None, ReadFlag::LargeInsertSize)}
				}		
				Some(MapPos{tid: mtid as u32, pos: mpos as u32})
			}
		} else { None };
		let maps = maps_from_bam_rec(sam_hdr, brec, keep_supplementary).unwrap_or_else(|e| panic!("Couldn't get map record from BAM read (id: {}: {}", brec.qname(), e));		
		let seq_qual = brec.get_seq_qual().unwrap_or_else(|e| panic!("Error in BAM: (id: {} - {})", brec.qname(), e));
		(Some(ReadEnd{seq_qual, maps, mate_pos, mate_idx: None }), read_flag)
	}

	fn maps_eq(&self, maps: &[Map]) -> bool {
		if self.maps.len() == maps.len() {
			for (m1, m2) in self.maps.iter().zip(maps.iter()) {	if m1 != m2 { return false } }
			true
		} else { false }
	}
	
	pub fn check_dup(&self, read_list: &[Option<Self>]) -> bool {
		for rd in read_list.iter() {
			if let Some(r) = rd {
				if self.maps_eq(&r.maps) && match (r.mate_pos, self.mate_pos) {
					(Some(x), Some(y)) => x == y,
					_ => true,
				} { return true }
			}
		}
		false
	}
	pub fn check_pair(&mut self, read: &mut Self, conf_hash: &ConfHash) -> (bool, ReadFlag) {
		let keep_unmatched = conf_hash.get_bool("keep_unmatched");
		let map1 = &mut self.maps[0];
		let map2 = &mut read.maps[0];
		// Check if maps are to the same contig
		if map1.map_pos.tid != map2.map_pos.tid { return (!keep_unmatched, ReadFlag::MisMatchContig) }
		// Check if maps are on different strands
		if ((map1.flags ^ map2.flags) & MFLAG_REVERSE) == 0 { return (!keep_unmatched, ReadFlag::BadOrientation) }
		// Check direction and template length
		if (map1.flags & MFLAG_REVERSE) == 0 {
			// Forward - Reverse
			let max_template_length = conf_hash.get_int("max_template_length") as u32;
			let end2 = map2.end();
			if end2 - map1.start() >= max_template_length { return (!keep_unmatched, ReadFlag::LargeInsertSize) }	
			// Check for overlap
			let end1 = map1.end();
			if end1 >= map2.start() {
				// If so then trim second read
				let trim = end1 - map2.start() + 1;
				map2.cigar.trim_start(trim);
				map2.map_pos.pos += trim;
			}
//			let tlen = (end2 as isize) - (map1.map_pos.pos as isize) + 1;
		} else {
			// Reverse - Forward: Can occur when the template length < read length.  In this case the reads should overlap
			let end1 = map1.end();
			if end1 < map2.start() { return (!keep_unmatched, ReadFlag::BadOrientation) }
			let end2 = map2.end();
			if map2.start() > map1.start() { 
				let trim = map2.start() - map1.start();
				map1.cigar.trim_start(trim);
				map1.map_pos.pos += trim;
			}
			if end2 > end1 { map2.cigar.trim_end(end2 - end1)}
		}
		(false, ReadFlag::Passed)
	}
	pub fn read_one(&self) -> bool { (self.maps[0].flags & (MFLAG_READ1 | MFLAG_READ2)) == MFLAG_READ1 }
	pub fn read_two(&self) -> bool { (self.maps[0].flags & (MFLAG_READ1 | MFLAG_READ2)) == MFLAG_READ2 }
	pub fn is_supplementary(&self) -> bool { (self.maps[0].flags & MFLAG_SPLIT) != 0 }
}

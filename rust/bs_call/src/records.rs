use std::str::{FromStr, from_utf8};
use super::htslib::*;
use super::config::ConfHash;

#[derive(Copy,Clone,PartialEq,Eq)]
pub struct MapPos {
	pub pos: u32,
	pub tid: u32,
}

pub const MFLAG_REVERSE:u16 = 4;
pub const MFLAG_SPLIT:u16 = 8;
pub const MFLAG_LAST: u16 = 16;
pub const MFLAG_DUPLICATE: u16 = 32;
pub const MFLAG_BSSTRAND_MASK:u16 = 3;

pub struct Map {
	pub map_pos: MapPos,
	pub cigar: CigarBuf,
	pub mapq: u8,
	pub split_id: u8,
	pub flags: u16,
}

const BSSTRAND_MAP:[BSStrand; 4] = [BSStrand::Unknown, BSStrand::StrandC2T, BSStrand::StrandG2A, BSStrand::Unconverted];
pub const BSS_UNKNOWN: u16 = 0;
pub const BSS_C2T: u16 = 1;
pub const BSS_G2A: u16 = 2;
pub const BSS_UNCONVERTED: u16 = 3;

impl Map {
	pub fn bs_strand(&self) -> BSStrand { BSSTRAND_MAP[(self.flags & MFLAG_BSSTRAND_MASK) as usize] }
}

fn maps_from_bam_rec(sam_hdr: &SamHeader, brec: &BamRec) -> Result<Vec<Map>, String> {
	// Handle basic fields
	let flag = brec.flag();
	let tst_flag = |x| (flag & x) != 0;
	if tst_flag(BAM_FUNMAP) { return Err("Read unmapped".to_string()) }
	let (bs_strand, sa_tag) = get_bs_strand(brec);
	let mut flags = bs_strand.get_num();
	if tst_flag(BAM_FDUP) { flags |= MFLAG_DUPLICATE }; 
	if tst_flag(BAM_FREVERSE) { flags |= MFLAG_REVERSE }; 
	let mut maps = Vec::with_capacity(1);
	let tid = if let Some(i) = brec.tid() {i as u32} else {return Err("Invalid contig ID".to_string())};
	let pos = if let Some(i) = brec.pos() {i as u32} else {return Err("Invalid map position".to_string())};
	let map_pos = MapPos{tid, pos};
	let cigar = if let Some(c) = brec.cigar() {c.to_cigar_buf()} else {return Err("No CIGAR field".to_string()) };
	maps.push(Map{map_pos, flags, mapq: brec.qual(), cigar, split_id: 0});
	// Handle SA tag
	if let Some(tag) = sa_tag.map(|s| from_utf8(s).unwrap() ) {
		for sa_map in tag.split(';') {
			if sa_map.trim().len() > 1 {
				maps.push(map_from_sa_tag(sam_hdr, sa_map)?);
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
	pub id: String,
	// For split mapped reads we get the details of all of the splits from the SA tag
	// The current mapping is always the first element in maps
	pub maps: Vec<Map>, 
	pub seq_qual: SeqQual,
	// For paired reads
	pub mate_pos: Option<MapPos>,
}

#[derive(Debug,Copy,Clone,PartialEq,Eq)]
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
		let mapq_threshold = conf_hash.get_int("mapq_threshold");
//		let max_template_length = conf_hash.get_int("max_template_length");
		
		// Check Bam record
		let flag = brec.flag();
		let mut read_flag = ReadFlag::None;
		let tst_flag = |x| (flag & x) != 0;
		let mut set_read_flag = |x| if read_flag == ReadFlag::None { read_flag = x };
		// Filter out the simple cases
		if tst_flag(BAM_FUNMAP) { return (None, ReadFlag::Unmapped)};
		if tst_flag(BAM_FQCFAIL) { return (None, ReadFlag::QCFail)};
		if tst_flag(BAM_FSECONDARY) { return (None, ReadFlag::Secondary)};
		if !ignore_duplicates && tst_flag(BAM_FDUP) { 
			if keep_duplicates { set_read_flag(ReadFlag::Duplicate) } else { return (None, ReadFlag::Duplicate)}
		}
		if tst_flag(BAM_FSUPPLEMENTARY) {
			if keep_supplementary { set_read_flag(ReadFlag::Supplementary) }  else {return (None, ReadFlag::Supplementary)}
		}
		if (brec.qual() as usize) < mapq_threshold { return (None, ReadFlag::LowMapQ)};
		if brec.tid().is_none() { panic!("Error in BAM: no contig id for mapped read (id: {})", brec.qname())};
		let mate_pos = if tst_flag(BAM_FPAIRED) {
			if tst_flag(BAM_FMUNMAP) { 
				if keep_unmatched { set_read_flag(ReadFlag::MateUnmapped)} else {return (None, ReadFlag::MateUnmapped)}
			} 
			let mtid = brec.mtid().unwrap_or_else(|| panic!("Error in BAM: no mate contig id for mapped mate (id: {})", brec.qname()));
			let mpos = brec.mpos().unwrap_or_else(|| panic!("Error in BAM: no mate position for mapped mate (id: {})", brec.qname()));
			Some(MapPos{tid: mtid as u32, pos: mpos as u32})
		} else { None };
		let maps = maps_from_bam_rec(sam_hdr, brec).unwrap_or_else(|e| panic!("Couldn't get map record from BAM read (id: {}: {}", brec.qname(), e));		
		let seq_qual = brec.get_seq_qual().unwrap_or_else(|e| panic!("Error in BAM: (id: {} - {})", brec.qname(), e));
		let id = brec.qname().to_owned();
		(Some(ReadEnd{id, seq_qual, maps, mate_pos}), read_flag)
	}
}

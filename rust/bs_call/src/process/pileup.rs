use std::sync::{Arc, mpsc};
use std::{cmp, io};

use crate::config::{BsCallConfig, BsCallFiles};
use crate::defs::CtgRegion;
use super::records::ReadEnd;
use crate::htslib::{Sequence, Faidx, CigarOp, hts_err, SeqQual, BSStrand};
use crate::stats::{StatJob, FSBaseLevelType, FSType};

pub struct PileupRegion {
	start: usize,
	end: usize,
	sam_tid: usize,
	cname: String,
	reads: Vec<Option<ReadEnd>>,
}

impl PileupRegion {
	pub fn new<S: AsRef<str>>(cname: S, start: usize, end: usize, sam_tid: usize, reads: Vec<Option<ReadEnd>>) -> Self {
		Self{cname: cname.as_ref().to_owned(), start, end, sam_tid, reads}
	}	
}

pub struct PileupPos {
	counts: [u32; 16],
	quality: [f32; 8],
	mapq2: f32	
}

const TAB_UNCONV: [usize; 4] = [0, 1, 2, 3];
const TAB_C2T: [usize; 4] = [0, 5, 2, 7];
const TAB_G2A: [usize; 4] = [4, 1, 6, 3];

impl PileupPos {
	pub fn new() -> Self { Self{counts: [0; 16], quality: [0.0; 8], mapq2: 0.0} }
	
	fn add_obs(&mut self, x: u8, rev: bool, bs: BSStrand, min_qual: u8, mapq2: f32) -> usize {
		let qual = x >> 2;
		if qual < min_qual { 1 }
		else {
			let base = (x & 3) as usize;
			let c = match bs {
				BSStrand::StrandC2T => TAB_C2T[base],
				BSStrand::StrandG2A => TAB_G2A[base],
				_ => TAB_UNCONV[base],
			};
			self.quality[c] += qual as f32;
			self.counts[c + if rev { 8 } else { 0 }] += 1;
			self.mapq2 += mapq2;
			0
		}
	}
}
impl Default for PileupPos { fn default() -> Self { Self::new() }}

pub struct Pileup {
	data: Vec<PileupPos>,
	start: usize, 
}

impl Pileup {
	fn add_obs(&mut self, pos: usize, sq: &[u8], rev: bool, bs: BSStrand, min_qual: u8, mapq2: f32) -> usize {
		let mut lq_ct = 0;
		let max = self.start + self.data.len();
		for (i, x) in sq.iter().enumerate() {
			if i + pos >= self.start {
				let j = i + pos - self.start;
				if j < max { lq_ct += self.data[j].add_obs(*x, rev, bs, min_qual, mapq2) }
			}
		}
		lq_ct
	}
}

fn check_sequence(cname: &str, seq: &mut Option<Sequence>, ref_index: &Faidx) -> io::Result<()> {
	if let Some(sq) = seq {
		if sq.cname() != cname {
			// Drop old sequence before loading in new contig data
			drop(seq.take().unwrap());
			info!("Loading sequence data for {}", cname);
			*seq = Some(ref_index.fetch_seq(cname)?);				
		}
	} else {
		info!("Loading sequence data for {}", cname);
		*seq = Some(ref_index.fetch_seq(cname)?);		
	}
	Ok(())	
}

fn load_ref_seq(preg: &mut PileupRegion, seq: &Option<Sequence>) -> io::Result<(Vec<u8>, usize, usize)> {
	// Extend range if possible to allow the reference context to be displayed (unless we are at the ends of the chromosomes)
	// if the end coordinate is past the end of the chromosome, get_seq will silently adjust it to the actual end point
	// x and y will have the start and end points of the fetched sequence while preg.start and preg.end are the regions 
	// for which the pileup will be generated
	let x = if preg.start > 2 {preg.start - 2} else {0};
	let ref_seq = seq.as_ref().unwrap().get_seq(x, preg.end + 2)?.to_vec();
	let y = x + ref_seq.len() - 1;
	if preg.end > y { preg.end = y }
	Ok((ref_seq, x, y))
}

// Check that the region falls overlaps one of the predefined contig regions, and trim if necessary
fn check_regions(preg: &mut PileupRegion, regions: &[CtgRegion]) -> io::Result<()> {
	let (mut max, mut max_ix) = (0, 0);
	for (ix, reg) in regions.iter().enumerate() {
		if reg.sam_tid == preg.sam_tid {
			let x = cmp::max(preg.start, reg.start);
			let y = cmp::min(preg.end, reg.stop);
			if y >= x && y - x > max {
				max = y - x;
				max_ix = ix;
			}
		}
	}
	if max > 0 {
		preg.start = cmp::max(regions[max_ix].start, preg.start);
		preg.end = cmp::min(regions[max_ix].stop, preg.end);
		Ok(())
	} else { Err(hts_err("Pileup region does not overlap a contig region".to_string())) }
}

fn add_read_to_pileup(read: &ReadEnd, pileup: &mut Pileup, ltrim: usize, rtrim: usize, min_qual: u8) -> (usize, usize, usize, usize, usize) {
	let cigar = &read.maps[0].cigar;
	let sq = &read.seq_qual;
	let mut ref_pos = read.maps[0].map_pos.pos as usize;
	let (mut seq_pos, mut orig_seq_pos) = (0, 0);
	let rev = read.maps[0].is_reverse();
	let bs = read.maps[0].bs_strand();
	let mapq = read.maps[0].mapq as f32;
	let mapq2 = mapq * mapq;
	// Left and right trimming refers to the original direction of the reads
	// so for a reversed read we need to reverse the trimming
	let (t1, t2) = if rev { (rtrim, ltrim) } else { (ltrim, rtrim) };
	let (mut clipped, mut trimmed, mut overlap, mut low_qual) = (0, 0, 0, 0);
	
	// Get cut off for right trim using the original length of sequence (including hard clips if present)
	let total_len = cigar.qlen1() as usize;
	if t1 + t2 >= total_len { clipped = total_len }
	else {
		let right_cut = total_len - t2;
		let mut state = if t1 > 0 { 0 } else { 1 };
		for elem in cigar.iter() {
			let (op, l) = elem.op_pair();
			let mut len = l as usize;
			let op_type = elem.op_type1();
			// Before the left trim position
			if state == 0 {
				let mut add = len;
				if (op_type & 1) == 1 {
					if orig_seq_pos + len >= t1 {
						state = 1;
						add -= orig_seq_pos + len - t1;
					}
					orig_seq_pos += add;
					if op != CigarOp::HardClip { seq_pos += add }	
				}
				if (op_type & 2) != 0 { ref_pos += add }
				match op {
					CigarOp::HardClip | CigarOp::SoftClip => clipped += add,
					CigarOp::Overlap => overlap += add,
					_ => trimmed += add, 
				}
				len -= add;
			}
			// After left trim but before right trim
			if state == 1 {
				let mut add = len;
				if (op_type & 1) == 1 && orig_seq_pos + len >= right_cut {
					state = 2;
					add -= orig_seq_pos + len - right_cut;
				}
				if add > 0 { 
					match op {
						CigarOp::Match | CigarOp::Equal | CigarOp::Diff => {
							low_qual += pileup.add_obs(ref_pos, &sq[seq_pos..seq_pos + add], rev, bs, min_qual, mapq2);
						},
						CigarOp::Overlap => overlap += add,
						_ => (),
					} 
					if (op_type & 1) != 0 {
						orig_seq_pos += add;
						if op != CigarOp::HardClip { seq_pos += add }	
					}
					if (op_type & 2) != 0 { ref_pos += add }
					len -= add;
				}			
			}
			// After right trim
			if len > 0 && state == 2 {
				match op {
					CigarOp::HardClip | CigarOp::SoftClip => clipped += len,
					CigarOp::Overlap => overlap += len,
					_ => if (op_type & 1) != 0 { trimmed += len }, 
				}				
			}			
		}
	}
	(total_len, clipped, trimmed, overlap, low_qual)

}

fn handle_pileup(bs_cfg: Arc<BsCallConfig>, mut preg: PileupRegion, seq: &mut Option<Sequence>, 
	ref_index: &Faidx, stat_tx: &mpsc::Sender<StatJob>) -> io::Result<()> {
	if preg.reads.is_empty() {
		warn!("make_pileup received empty read vector");
		return Ok(())
	}
	let ltrim1 = bs_cfg.conf_hash.get_int("left_trim_read_1");
	let ltrim2 = bs_cfg.conf_hash.get_int("left_trim_read_2");
	let rtrim1 = bs_cfg.conf_hash.get_int("right_trim_read_1");
	let rtrim2 = bs_cfg.conf_hash.get_int("right_trim_read_2");
	let min_qual = bs_cfg.conf_hash.get_int("bq_threshold") as u8;
	check_sequence(&preg.cname, seq, ref_index)?;
	check_regions(&mut preg, &bs_cfg.regions)?;	
	let (ref_seq, ref_start, ref_end) = load_ref_seq(&mut preg, seq)?;
	let mut fs_stats = FSType::new();
	let size = preg.end + 1 - preg.start;
	let mut pileup_vec = Vec::with_capacity(size);
	for _ in 0..size { pileup_vec.push(PileupPos::new())}
	let mut pileup = Pileup{data: pileup_vec, start: preg.start };
	for rd in preg.reads.drain(..) {
		if let Some(read) = rd {
			let (ltrim, rtrim) = if read.read_one() { (ltrim1, rtrim1) }
			else if read.read_two() { (ltrim2, rtrim2) }
			else { (0, 0) };
			let (l, clipped, trimmed, overlap, low_qual) = add_read_to_pileup(&read, &mut pileup, ltrim, rtrim, min_qual);
			if read.is_primary() {
				if clipped > 0 { fs_stats.add_base_level_count(FSBaseLevelType::Clipped, clipped) };
				if overlap > 0 { fs_stats.add_base_level_count(FSBaseLevelType::Overlapping, overlap) };
				if trimmed > 0 { fs_stats.add_base_level_count(FSBaseLevelType::Trimmed, trimmed) };
				if low_qual > 0 { fs_stats.add_base_level_count(FSBaseLevelType::LowQuality, low_qual) };
				let nflt = clipped + overlap + trimmed + low_qual;
				if nflt < l { fs_stats.add_base_level_count(FSBaseLevelType::Passed, l - nflt) };
			}
		}
	}
	for (flag, ct) in fs_stats.base_level().iter() { let _ = stat_tx.send(StatJob::AddFSBaseLevelCounts(*flag, *ct)); }
	Ok(())
}

pub fn make_pileup(bs_cfg: Arc<BsCallConfig>, rx: mpsc::Receiver<Option<PileupRegion>>, mut bs_files: BsCallFiles, stat_tx: mpsc::Sender<StatJob>) {
	info!("pileup_thread starting up()");
	let ref_index = bs_files.ref_index.take().unwrap();
	let mut seq: Option<Sequence> = None;
	loop {
		match rx.recv() {
			Ok(None) => break,
			Ok(Some(preg)) => {
				debug!("Received new pileup region: {}:{}-{}", preg.cname, preg.start, preg.end);
				if let Err(e) = handle_pileup(Arc::clone(&bs_cfg), preg, &mut seq, &ref_index, &stat_tx) {
					error!("handle_pileup failed with error: {}", e);
					break;
				}
			},
			Err(e) => {
				warn!("make_pileup_thread recieved error: {}", e);
				break
			}
		}
	}
	info!("pileup_thread shutting down");
}	

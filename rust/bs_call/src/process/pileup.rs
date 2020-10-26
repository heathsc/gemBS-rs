use std::sync::{Arc, mpsc};
use std::{cmp, io, thread, slice};

use crate::config::{BsCallConfig, BsCallFiles};
use crate::defs::CtgRegion;
use super::records::ReadEnd;
use super::call_genotypes;
use crate::htslib::{Sequence, Faidx, CigarOp, hts_err, BSStrand};
use crate::stats::{StatJob, FSBaseLevelType, FSType, MethProfile};
use crate::dbsnp::{DBSnpFile, DBSnpContig};

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
	pub counts: [u32; 16],
	pub quality: [f32; 8],
	pub mapq2: f32	
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
	pub data: Vec<PileupPos>,
	pub ref_seq: Vec<u8>,
	pub gc_bins: Vec<u8>,
	pub dbsnp_contig: Option<DBSnpContig>, 
	pub start: usize, 
	pub ref_start: usize,
	pub sam_tid: usize,
}

impl Pileup {
	fn new(data: Vec<PileupPos>, ref_seq: Vec<u8>, start: usize, ref_start: usize, gc_bins: Vec<u8>, sam_tid: usize, cname: &str, dbsnp_file: &mut Option<DBSnpFile>) -> Self {
		assert!(start >= ref_start);
		assert!(start - ref_start <= 2);
		let dbsnp_contig = if let Some(x) = dbsnp_file.as_ref() { x.get_dbsnp_contig(cname) } else { None };
		Self{data, ref_seq, gc_bins, start, ref_start, sam_tid, dbsnp_contig}
	}
	
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
	
	pub fn get_prec_2_bases(&self) -> [u8; 2] {
		match self.start - self.ref_start {
			0 => [0, 0],
			1 => [0, self.ref_seq[0]],
			2 => [self.ref_seq[0], self.ref_seq[1]],
			_ => panic!("Illegal configuration"),
		}
	}
	
	pub fn get_ref_iter(&self) -> slice::Iter<u8> {	self.ref_seq[self.start - self.ref_start..].iter() }
}

fn send_call_job(pileup: Pileup, call_tx: &mpsc::SyncSender<Option<Pileup>>) -> io::Result<()> {
	match call_tx.send(Some(pileup)) { 
		Err(e) => {
			warn!("Error trying to send new region to call_genotypes thread");
			Err(hts_err(format!("Error sending region to call_genotypes thread: {}", e)))
		},
		Ok(_) => Ok(()),
	} 	
}

pub const GC_BIN_SIZE: u8 = 100;

const GC_TAB: [[u8; 2]; 256] = [
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [1, 0], [0, 0], [1, 1], [0, 0], [0, 0], [0, 0], [1, 1], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [1, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [1, 0], [0, 0], [1, 1], [0, 0], [0, 0], [0, 0], [1, 1], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [1, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
	[0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], [0, 0], 
];

pub struct GcContent {
	bins: Vec<u8>,
	bin_size: u8,
}

impl GcContent {
	fn generate_bins(seq: &Sequence) -> Self {
		let bin_size = GC_BIN_SIZE;
		let mut bins = Vec::with_capacity(1 + seq.len() / (bin_size as usize));
		let (mut ix, mut a, mut b) = (0, 0, 0);
		for c in seq.get_seq(0, seq.len() - 1).unwrap().iter() {
			let delta = GC_TAB[*c as usize];
			a += delta[0];
			b += delta[1];
			ix += 1;
			if ix == bin_size {
				bins.push(if a == bin_size { b } else { 255 });
				ix = 0;
				a = 0;
				b = 0;
			}
		}
		if ix > 0 { bins.push(255); }
		Self{bins, bin_size: GC_BIN_SIZE}
	}
	fn get_gc_slice(&self, x: usize, y: usize) -> &[u8] {
		let x1 = x / (self.bin_size as usize);
		let y1 = cmp::min(y / (self.bin_size as usize) + 1, self.bins.len() - 1);
		 &self.bins[x1..=y1]
	}
}

fn check_sequence(cname: &str, seq: &mut Option<SeqData>, ref_index: &Faidx, dbsnp_file: &mut Option<DBSnpFile>) -> io::Result<()> {
	if let Some(seq_data) = seq {
		if seq_data.seq.cname() != cname { 
			if let Some(dbsnp) = dbsnp_file { dbsnp.unload_ctg(seq_data.seq.cname()); }
			seq.take(); 
		}
	}
	if seq.is_none() {
		if let Some(dbsnp) = dbsnp_file { dbsnp.load_ctg(cname)?; } 
		info!("Loading sequence data for {}", cname);
		let new_seq = ref_index.fetch_seq(cname)?;
		let gc_content = GcContent::generate_bins(&new_seq);
		*seq = Some(SeqData{seq: new_seq, gc_content});
		info!("Sequence data loaded");
	}
	Ok(())
}

const REF_TAB: [u8; 256] = [
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 1, 0, 2, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 1, 0, 2, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,	
];

fn load_ref_seq(preg: &mut PileupRegion, seq_data: &Option<SeqData>) -> io::Result<(Vec<u8>, usize, Vec<u8>)> {
	// Extend range if possible to allow the reference context to be displayed (unless we are at the ends of the chromosomes)
	// if the end coordinate is past the end of the chromosome, get_seq will silently adjust it to the actual end point
	// x and y will have the start and end points of the fetched sequence while preg.start and preg.end are the regions 
	// for which the pileup will be generated.
	// The returned sequence is copied to a vector and translated A=1, C=2, G=3, T=4, _=0
	
	let x = if preg.start > 2 {preg.start - 2} else {0};
	let seq_data = seq_data.as_ref().unwrap();
	
	let ref_seq: Vec<_> = seq_data.seq.get_seq(x, preg.end + 2)?.iter().map(|c| REF_TAB[*c as usize]).collect();
	let y = x + ref_seq.len() - 1;
	if preg.end > y { preg.end = y }
	let gc_bins = seq_data.gc_content.get_gc_slice(x, y).to_vec();
	Ok((ref_seq, x, gc_bins))
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

fn add_read_to_pileup(read: &ReadEnd, pileup: &mut Pileup, ltrim: usize, rtrim: usize, min_qual: u8, mprof: &mut MethProfile) -> (usize, usize, usize, usize, usize, usize) {
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
	let (mut clipped, mut trimmed, mut overlap, mut low_qual, mut inserts) = (0, 0, 0, 0, 0);
	
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
					CigarOp::Ins => inserts += add,
					_ => if (op_type & 1) != 0 { trimmed += add }, 
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
							let opos = if rev {
								assert!(orig_seq_pos + add <= total_len);
								total_len - 1 - orig_seq_pos
							} else { orig_seq_pos };
							// Get previous reference base
							let state = if ref_pos > pileup.ref_start { pileup.ref_seq[ref_pos - 1 - pileup.ref_start] } else { 0 };
							mprof.add_profile(&pileup.ref_seq[ref_pos - pileup.ref_start..], opos as isize, state, &sq[seq_pos..seq_pos + add], rev, bs);
						},
						CigarOp::Overlap => overlap += add,
						CigarOp::HardClip | CigarOp::SoftClip => clipped += add,
						CigarOp::Ins => inserts += add,
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
					CigarOp::Ins => inserts += len,
					_ => if (op_type & 1) != 0 { trimmed += len }, 
				}				
			}			
		}
	}
//	if total_len < (clipped + trimmed + overlap + low_qual + inserts) {
//		println!("len: {}, clp: {}, trm: {}, over: {}, lq: {}, ins: {}, cigar: {}", total_len, clipped, trimmed, overlap, low_qual, inserts, cigar);
//		println!("t1: {}, t2: {}, right_cut: {}", t1, t2, total_len - t2);
//	}
	(total_len, clipped, trimmed, overlap, low_qual, inserts)

}

fn handle_pileup(pileup_data: &mut PileupData, mut preg: PileupRegion, stat_tx: &mpsc::Sender<StatJob>, call_tx: &mpsc::SyncSender<Option<Pileup>>) -> io::Result<()> {
	if preg.reads.is_empty() {
		warn!("make_pileup received empty read vector");
		return Ok(())
	}
	let bs_cfg = &pileup_data.bs_cfg;
	let ltrim1 = bs_cfg.conf_hash.get_int("left_trim_read_1");
	let ltrim2 = bs_cfg.conf_hash.get_int("left_trim_read_2");
	let rtrim1 = bs_cfg.conf_hash.get_int("right_trim_read_1");
	let rtrim2 = bs_cfg.conf_hash.get_int("right_trim_read_2");
	let min_qual = bs_cfg.conf_hash.get_int("bq_threshold") as u8;
	let seq_data = &mut pileup_data.seq_data;
	let ref_index = &pileup_data.ref_index;
	let dbsnp_file = &mut pileup_data.dbsnp_file;
	check_sequence(&preg.cname, seq_data, ref_index, dbsnp_file)?;
	check_regions(&mut preg, &bs_cfg.regions)?;	
	let meth_prof = &mut pileup_data.meth_prof;
	let (ref_seq, ref_start, gc_bins) = load_ref_seq(&mut preg, &seq_data)?;
	let mut fs_stats = FSType::new();
	let size = preg.end + 1 - preg.start;
	let mut pileup_vec = Vec::with_capacity(size);
	for _ in 0..size { pileup_vec.push(PileupPos::new())}
	let mut pileup = Pileup::new(pileup_vec, ref_seq, preg.start, ref_start, gc_bins, preg.sam_tid, &preg.cname, dbsnp_file);
	for rd in preg.reads.drain(..) {
		if let Some(read) = rd {
			let (ltrim, rtrim) = if read.read_one() { (ltrim1, rtrim1) }
			else if read.read_two() { (ltrim2, rtrim2) }
			else { (0, 0) };
			let (l, clipped, trimmed, overlap, low_qual, inserts) = add_read_to_pileup(&read, &mut pileup, ltrim, rtrim, min_qual,  meth_prof);
			if read.is_primary() {
				if clipped > 0 { fs_stats.add_base_level_count(FSBaseLevelType::Clipped, clipped) };
				if overlap > 0 { fs_stats.add_base_level_count(FSBaseLevelType::Overlapping, overlap) };
				if trimmed > 0 { fs_stats.add_base_level_count(FSBaseLevelType::Trimmed, trimmed) };
				if low_qual > 0 { fs_stats.add_base_level_count(FSBaseLevelType::LowQuality, low_qual) };
				if inserts > 0 { fs_stats.add_base_level_count(FSBaseLevelType::Inserts, low_qual) };
				let nflt = clipped + overlap + trimmed + low_qual + inserts;
				assert!(nflt <= l);
				fs_stats.add_base_level_count(FSBaseLevelType::Passed, l - nflt);
			}
		}
	}
	send_call_job(pileup, &call_tx)?;	
	for (flag, ct) in fs_stats.base_level().iter() { let _ = stat_tx.send(StatJob::AddFSBaseLevelCounts(*flag, *ct)); }
	Ok(())
}

pub struct SeqData {
	seq: Sequence,
	gc_content: GcContent,
}

pub struct PileupData {
	seq_data: Option<SeqData>,
	ref_index: Faidx,
	dbsnp_file: Option<DBSnpFile>,
	bs_cfg: Arc<BsCallConfig>,
	meth_prof: MethProfile,
}

pub fn make_pileup(bs_cfg: Arc<BsCallConfig>, rx: mpsc::Receiver<Option<PileupRegion>>, mut bs_files: BsCallFiles, stat_tx: mpsc::Sender<StatJob>) {
	info!("pileup_thread starting up");
	let dbsnp_file = if let Some(dbsnp_index) = bs_files.dbsnp_index.take() {
		match DBSnpFile::open(dbsnp_index) {
			Ok(db) => Some(db),
			Err(e) => {
				error!("Couldn't open dbSNP index file: {}", e);
				None
			},
		}	
	} else { None }; 
	let min_qual = bs_cfg.conf_hash.get_int("bq_threshold") as u8;
	let mut pileup_data = PileupData {
		seq_data: None,
		ref_index: bs_files.ref_index.take().unwrap(),
		dbsnp_file,
		meth_prof: MethProfile::new(min_qual as usize),
		bs_cfg: Arc::clone(&bs_cfg),
	};
	let (call_tx, call_rx) = mpsc::sync_channel(32);
	let st_tx = mpsc::Sender::clone(&stat_tx);
	let call_handle = thread::spawn(move || { call_genotypes::call_genotypes(Arc::clone(&bs_cfg), call_rx, bs_files, st_tx) });
	loop {
		match rx.recv() {
			Ok(None) => break,
			Ok(Some(preg)) => {
				debug!("Received new pileup region: {}:{}-{}", preg.cname, preg.start, preg.end);
				if let Err(e) = handle_pileup(&mut pileup_data, preg, &stat_tx, &call_tx) {
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
	if call_tx.send(None).is_err() { warn!("Error trying to send QUIT signal to call_genotypes thread") }
	else {
		let _ = stat_tx.send(StatJob::SetNonCpgReadProfile(pileup_data.meth_prof.take_profile()));
		if call_handle.join().is_err() { warn!("Error waiting for call_genotype thread to finish") }
	}
	info!("pileup_thread shutting down");
}	

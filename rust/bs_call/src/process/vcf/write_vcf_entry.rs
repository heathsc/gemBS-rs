use std::sync::{Arc, mpsc};
use std::{cmp, io};
use std::collections::VecDeque;
use crate::config::{BsCallConfig, BsCallFiles};
use crate::htslib::*;
use libc::{c_char, c_int};
use crate::stats::StatJob;
use crate::process::call_genotypes::{CallBlock, GenotypeCall, CallEntry};

pub enum WriteVcfJob {
	CallBlock(CallBlock),
	GenotypeCall(Vec<CallEntry>),
	Quit,
}

use std::f64::consts::LN_10;

const FLT_NAMES: [&str; 16] = ["PASS", "mac1", "fail", "GT", "FT", "DP", "MQ", "GQ", "QD", "GL", "MC8", "AMQ", "CS", "CG", "CX", "FS" ];	
const FLT_ID_PASS: usize = 0;
const FLT_ID_MAC1: usize = 1;
const FLT_ID_FAIL: usize = 2;
const FLT_ID_GT: usize = 3;
const FLT_ID_FT: usize = 4;
const FLT_ID_DP: usize = 5;
const FLT_ID_MQ: usize = 6;
const FLT_ID_GQ: usize = 7;
const FLT_ID_QD: usize = 8;
const FLT_ID_GL: usize = 9;
const FLT_ID_MC8: usize = 10;
const FLT_ID_AMQ: usize = 11;
const FLT_ID_CS: usize = 12;
const FLT_ID_CG: usize = 13;
const FLT_ID_CX: usize = 14;
const FLT_ID_FS: usize = 15;

fn get_filter_ids(hdr: &VcfHeader) -> Vec<u8> {
	let mut v = Vec::with_capacity(FLT_NAMES.len());
	for flt in FLT_NAMES.iter() {
		v.push(hdr.id2int(BCF_DT_ID as usize, flt).expect("mac1 filter not found in VCF header") as u8)
	}	
	v
}

// Whether to output a site based on called genotype (0-9) and reference base [N, A, C, G, T]
const GT_FLAG: [[bool; 5]; 10] = [
	[true, false, true, true, true], // AA
	[true, true, true, true, true],  // AC
	[true, true, true, true, true],  // AG
	[true, true, true, true, true],  // AT
	[true, true, true, true, true],  // CC
	[true, true, true, true, true],  // CG
	[true, true, true, true, true],  // CT
	[true, true, true, true, true],  // GG
	[true, true, true, true, true],  // GT
	[true, true, true, true, false], // TT
];

// Alternate alleles as a function of called genotype [0-9] and reference base [N, A, C, G, T]
const REF_ALT: [[&str; 5]; 10] = [
		["A", "", "A", "A", "A"],     // AA
		["AC", "C", "A", "AC", "AC"], // AC
		["AG", "G", "AG", "A", "AG"], // AG
		["AT", "T", "AT", "AT", "A"], // AT
		["C", "C", "", "C", "C"],     // CC
		["CG", "CG", "G", "C", "CG"], // CG
		["CT", "CT", "T", "CT", "C"], // CT
		["G", "G", "G", "", "G"],     // GG
		["GT", "GT", "GT", "T", "G"], // GT
		["T", "T", "T", "T", ""]      // TT
];

const IUPAC: &str = "NAMRWCSYGKT"; 
const PBASE: &str = "NACGT"; 

const CPG_STATE: [[usize; 11] ;11] = [
//    ?? AA AC AG AT CC CG CT GG GC TT	
	[ 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0 ], // ??
	[ 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0 ], // AA
	[ 1, 2, 2, 3, 2, 2, 3, 2, 3, 3, 2 ], // AC
	[ 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0 ], // AG
	[ 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0 ], // AT
	[ 1, 2, 2, 3, 2, 2, 3, 2, 4, 3, 2 ], // CC
	[ 1, 2, 2, 3, 2, 2, 3, 2, 3, 3, 2 ], // CG
	[ 1, 2, 2, 3, 2, 2, 3, 2, 3, 3, 2 ], // CT
	[ 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0 ], // GG
	[ 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0 ], // GT
	[ 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0 ]  // TT
];

const CPG_DISPLAY: [u8; 5] = [ b'.', b'?', b'N', b'H', b'Y' ];
const CS_STR: [&str; 10] = ["NA", "+", "-", "NA", "+", "+-", "+", "-", "-",  "NA"];

const GT_INT: [[[c_int; 2]; 5]; 10] = [
	[[4, 4], [2, 2], [4, 4], [4, 4], [4, 4]], // AA
	[[4, 8], [2, 4], [2, 4], [4, 8], [4, 8]], // AC
	[[4, 8], [2, 4], [4, 8], [2, 4], [4, 8]], // AG
	[[4, 8], [2, 4], [4, 8], [4, 8], [2, 4]], // AT
	[[4, 4], [4, 4], [2, 2], [4, 4], [4, 4]], // CC
	[[4, 8], [4, 8], [2, 4], [2, 4], [4, 8]], // CG
	[[4, 8], [4, 8], [2, 4], [4, 8], [2, 4]], // CT
	[[4, 4], [4, 4], [4, 4], [2, 2], [4, 4]], // GG
	[[4, 8], [4, 8], [4, 8], [2, 4], [2, 4]], // GT
	[[4, 4], [4, 4], [4, 4], [4, 4], [2, 2]], // TT
];

const ALL_IDX: [[[usize; 2]; 5]; 10] = [
	[[1, 0], [0, 0], [1, 0], [1, 0], [1, 0]], // AA
	[[1, 2], [2, 0], [1, 0], [1, 2], [1, 2]], // AC
	[[1, 3], [3, 0], [1, 3], [1, 0], [1, 3]], // AG
	[[1, 4], [4, 0], [1, 4], [1, 4], [1, 0]], // AT
	[[2, 0], [2, 0], [0, 0], [2, 0], [2, 0]], // CC
	[[2, 3], [2, 3], [3, 0], [2, 0], [2, 3]], // CG
	[[2, 4], [2, 4], [4, 0], [2, 4], [2, 0]], // CT
	[[3, 0], [3, 0], [3, 0], [0, 0], [3, 0]], // GG
	[[3, 4], [3, 4], [3, 4], [4, 0], [3, 0]], // GT
	[[4, 0], [4, 0], [4, 0], [4, 0], [0, 0]], // GT
];

const GT_HET: [bool; 10] = [ false, true, true, true, false, true, true, false, true, false ];

const FLT_NAME: [&str; 4] = [ "q20", "qd2", "fs60", "mq40" ];

struct CallStats {
	phred: c_int,
	fs: c_int,
	qd: c_int,
	dp1: c_int,
}

fn ln_prob_2_phred(p: f64) -> c_int {
	let z = (p * LN_10).exp();
	if z >= 1.0 { 255 } 
	else { cmp::min((-10.0 * (1.0 - z).ln() / LN_10) as c_int, 255) }	
}

fn handle_filters(call: &GenotypeCall, cs: &CallStats) -> (usize, usize) {
	let flt = if cs.phred < 20 { 1 } else { 0 } 
		| if cs.qd < 2 { 2 } else { 0 }
		| if cs.fs > 60 { 4 } else { 0 }
		| if call.mq < 40 { 8 } else { 0 };
		
	if flt == 0 {
		let counts = &call.counts;
		if match call.max_gt {
			// AC
			1 => counts[1] + counts[5] + counts[7] <= 1 || counts[0] + counts[4] <= 1,
			// AG
			2 => counts[2] + counts[6] <= 1 || counts[0] <= 1,
			// AT
			3 => counts[3] + counts[7] <= 1 || counts[0] + counts[4] <= 1,
			// CG
			5 => counts[2] + counts[6] + counts[4] <= 1 || counts[1] + counts[5] + counts[7] <= 1,
			// CT
			6 => counts[3] <= 1 || counts[1] + counts[5] <= 1,
			// GT
			8 => counts[3] + counts[7] <= 1 || counts[2] + counts[6] + counts[4] <= 1,
			// All homozygous genotypes
			_ => false,
		} {
			(128, FLT_ID_MAC1)
		} else { (0, FLT_ID_PASS) }
	} else { (flt, FLT_ID_FAIL) }
}

fn get_filter_string(filter: usize) -> String {
	if (filter & 15) != 0 {
		let mut flt1 = filter & 15;
		let mut tstr = String::with_capacity(32);
		for f in FLT_NAME.iter() {
			if (flt1 & 1) != 0 {
				if !tstr.is_empty() { tstr.push(';') }
				tstr.push_str(f);
			}
			flt1 >>= 1;
			if flt1 == 0 { break }	
		}
		tstr
	} else { "PASS".to_string() }
}

fn get_gt_like(call: &GenotypeCall) -> Vec<f32> {
	let mut v = Vec::with_capacity(6);
	let rfix = call.ref_base as usize;
	let aix = &ALL_IDX[call.max_gt as usize][rfix];
	let get_z = |a, b| { 
		let j = if a < b { a * (9 - a) / 2 + b - 5 } else { b * (9 - b) / 2 + a - 5};
		if call.gt_ll[j] < -99.999 { -99.999 } else { call.gt_ll[j] as f32}
	};
	 
	v.push(if rfix != 0 { get_z(rfix, rfix) } else { -99.999 }); 
	if aix[0] > 0 {
		if rfix > 0 { v.push(get_z(rfix, aix[0])) }
		v.push(get_z(aix[0], aix[0]));
		if aix[1] > 0 {
			if rfix > 0 { v.push(get_z(rfix, aix[1])) }
			v.push(get_z(aix[0], aix[1]));
			v.push(get_z(aix[1], aix[1]));
		}
	}
	v
}

fn enc_size(v: &mut Vec<u8>, size: c_int, bcf_type: u8) {
	if size < 15 { v.push(((size as u8) << 4) | bcf_type) }
	else {
		v.push(0xf0 | bcf_type);
		if size >= 128 {
			if size >= 32768 {
				v.push(0x10 | BCF_BT_INT32);
				v.extend_from_slice(&size.to_le_bytes());
			} else {
				v.push(0x10 | BCF_BT_INT16);
				v.extend_from_slice(&(size as u16).to_le_bytes());
			}
		} else {
			v.push(0x10 | BCF_BT_INT8);
			v.push(size as u8);
		}
	}
}

// Note - we don't check for vector_end or missing values here as we don't generate these
fn enc_int(v: &mut Vec<u8>, x: c_int) {
	if x <= BCF_MAX_BT_INT8 && x >= BCF_MIN_BT_INT8 {
		enc_size(v, 1, BCF_BT_INT8);
		v.push(x as u8)
	} else if x <= BCF_MAX_BT_INT16 && x >= BCF_MIN_BT_INT16 {
		enc_size(v, 1, BCF_BT_INT16);
		v.extend_from_slice(&(x as u16).to_le_bytes())							
	} else {
		enc_size(v, 1, BCF_BT_INT32);
		v.extend_from_slice(&(x as u32).to_le_bytes())							
	}	
}

#[inline]
fn enc_u8(v: &mut Vec<u8>, x: u8) {
	v.push(0x10 | BCF_BT_INT8);
	v.push(x);
}

fn enc_vchar(v: &mut Vec<u8>, s: &[u8]) { 
	enc_size(v, s.len() as c_int, BCF_BT_CHAR);
	v.extend_from_slice(s) 
}

fn enc_vfloat(v: &mut Vec<u8>, s: &[f32]) {
	enc_size(v, s.len() as c_int, BCF_BT_FLOAT);
	s.iter().copied().for_each(|x| v.extend_from_slice(&x.to_le_bytes()));
}

fn enc_vint(v: &mut Vec<u8>, s: &[c_int]) {
	if s.len() == 1 { enc_int(v, s[0])}
	else {
		let (min, max): (c_int, c_int) = s[1..].iter().copied().fold((s[0], s[0]), |(mn, mx), x| if x < mn { (x, mx) } else if x > mx { (mn, x) } else { (mn, mx) });
		if max <= BCF_MAX_BT_INT8 && min >= BCF_MIN_BT_INT8 {
			enc_size(v, s.len() as c_int, BCF_BT_INT8);
			s.iter().copied().for_each(|x| v.push(x as u8));
		} else if max <= BCF_MAX_BT_INT16 && min >= BCF_MIN_BT_INT16 {
			enc_size(v, s.len() as c_int, BCF_BT_INT16);
			s.iter().copied().for_each(|x| v.extend_from_slice(&(x as u16).to_le_bytes()));
		} else {
			enc_size(v, s.len() as c_int, BCF_BT_INT32);
			s.iter().copied().for_each(|x| v.extend_from_slice(&(x as u32).to_le_bytes()));							
		}
	}		
}

fn write_fixed_columns(call: &GenotypeCall, filter_ids: &[u8], v: &mut Vec<u8>, call_stats: &CallStats, ref_context: &[u8], bcf_rec: &mut BcfRec) -> io::Result<usize> {
	v.clear();
	// Alternate alleles
	let alt_alleles = REF_ALT[call.max_gt as usize][call.ref_base as usize];
	// ID - for the moment leave blank
	v.push(BCF_BT_CHAR);	
	// REF allele
	v.push(0x10 | BCF_BT_CHAR);
	v.push(ref_context[2]);
	// ALT alleles
	for allele in alt_alleles.as_bytes().iter().copied() {
		v.push(0x10 | BCF_BT_CHAR);
		v.push(allele);
	}
	// FILTER
	let (filter, filter_id) = handle_filters(call, &call_stats);	
	enc_u8(v, filter_ids[filter_id]);	
	// INFO
	enc_u8(v, filter_ids[FLT_ID_CX]);
	enc_vchar(v, ref_context);	

	let kstr = bcf_rec.shared();
	kstr.putsn(v.as_slice().as_ptr() as *const c_char, v.len());

	bcf_rec.set_n_sample(1);
	bcf_rec.set_rlen(1);	
	bcf_rec.set_n_allele(1 + alt_alleles.len());
	bcf_rec.set_qual(call_stats.phred as f32);	
	bcf_rec.set_n_info(1);
	Ok(filter)
}

fn write_format_columns(call: &GenotypeCall, filter_ids: &[u8], called_context: &[u8], cpg: u8, filter: usize, v: &mut Vec<u8>, call_stats: &CallStats, bcf_rec: &mut BcfRec) -> io::Result<()> {
	let mut n_fmt = 11;
	v.clear();
	// GT
	enc_u8(v, filter_ids[FLT_ID_GT]);
	enc_vint(v, &GT_INT[call.max_gt as usize][call.ref_base as usize]);
	// FT
	let flt_str = get_filter_string(filter);
	enc_u8(v, filter_ids[FLT_ID_FT]);
	enc_vchar(v, flt_str.as_bytes());

	// DP
	enc_u8(v, filter_ids[FLT_ID_DP]);
	enc_int(v, call_stats.dp1);
	// MQ
	enc_u8(v, filter_ids[FLT_ID_MQ]);
	enc_int(v, call.mq as c_int);
	// GQ
	enc_u8(v, filter_ids[FLT_ID_GQ]);
	enc_int(v, call_stats.phred);
	// QD
	enc_u8(v, filter_ids[FLT_ID_QD]);
	enc_int(v, call_stats.qd);
	// GL
	let gl = get_gt_like(call);	
	enc_u8(v, filter_ids[FLT_ID_GL]);
	enc_vfloat(v, &gl);
	// MC8
	enc_u8(v, filter_ids[FLT_ID_MC8]);
	enc_vint(v, &call.counts);
	// AMQ
	let tv: Vec<c_int> = call.counts.iter().copied().zip(call.qual.iter()).filter(|(c,_)| *c > 0).map(|(_, q)| *q).collect();
	if !tv.is_empty() {
		enc_u8(v, filter_ids[FLT_ID_AMQ]);
		enc_vint(v, &tv);
		n_fmt += 1;
	}
	// CS
	enc_u8(v, filter_ids[FLT_ID_CS]);
	enc_vchar(v, CS_STR[call.max_gt as usize].as_bytes());
	// CG
	enc_u8(v, filter_ids[FLT_ID_CG]);
	v.push(0x10 | BCF_BT_CHAR);
	v.push(cpg);
	// CX
	enc_u8(v, filter_ids[FLT_ID_CX]);
	enc_vchar(v, called_context);
	// FS
	if GT_HET[call.max_gt as usize] {
		enc_u8(v, filter_ids[FLT_ID_FS]);
		enc_int(v, call_stats.fs);
		n_fmt += 1;		
	}
	let kstr = bcf_rec.indiv();
	kstr.putsn(v.as_slice().as_ptr() as *const c_char, v.len());
	bcf_rec.set_n_fmt(n_fmt);
	Ok(())
}

struct WriteState {
	sam_tid: usize,
	curr_x: usize,
	call_buf: VecDeque<CallEntry>,
	bcf_rec: BcfRec,
	tvec: Vec<u8>,
}

impl WriteState {
	fn new_block(call_block: CallBlock) -> Self {
		let mut v = VecDeque::with_capacity(5);
		for _ in 0..3 { v.push_back(CallEntry::Starting(0)) }
		for c in call_block.prec_ref_bases.iter() { v.push_back(CallEntry::Starting(*c)) }
		let bcf_rec = BcfRec::new().expect("Couldn't allocate Bcf Record");
		let tvec = Vec::with_capacity(256);
		Self { sam_tid: call_block.sam_tid, curr_x: call_block.start, call_buf: v, bcf_rec, tvec}
	}
	fn finish_block(&mut self, bs_cfg: &BsCallConfig, vcf_output: &mut VcfFile, filter_ids: &[u8], stat_tx: &mpsc::Sender<StatJob>) -> io::Result<()> {
		for _ in 0..2 {
			self.add_entry(CallEntry::Skip(0));
			self.write_entry(bs_cfg, vcf_output, filter_ids, stat_tx)?;
		}
		Ok(())
	}
	fn handle_calls(&mut self, mut call_vec: Vec<CallEntry>, bs_cfg: &BsCallConfig, vcf_output: &mut VcfFile, filter_ids: &[u8], stat_tx: &mpsc::Sender<StatJob>) -> io::Result<()> {
		for entry in call_vec.drain(..) {
			self.add_entry(entry);
			self.write_entry(bs_cfg, vcf_output, filter_ids, stat_tx)?;
		}
		Ok(())
	}
	fn add_entry(&mut self, entry: CallEntry) {
		assert!(self.call_buf.len() == 5);
		self.call_buf.push_back(entry);
		let _ = self.call_buf.pop_front();		
	}
	fn write_entry(&mut self, bs_cfg: &BsCallConfig, vcf_output: &mut VcfFile, filter_ids: &[u8], stat_tx: &mpsc::Sender<StatJob>) -> io::Result<()> {
		match &self.call_buf[2] {
			CallEntry::Call(call) => {
				let all_positions = bs_cfg.conf_hash.get_bool("all_positions");
				let dp1: c_int = call.counts[0..4].iter().sum();
				let d_inf: c_int = call.counts[4..].iter().sum();
				// Skip sites with no coverage
				if dp1 + d_inf == 0 { return Ok(()) }
				let phred = ln_prob_2_phred(call.gt_ll[call.max_gt as usize]);
				// Fisher strand and quality by depth stats
				let fs = (call.fisher_strand * -10.0 + 0.5).round() as c_int;
				let qd = if dp1 > 0 { phred / dp1 } else { phred };
				let call_stats = CallStats{phred, fs, dp1, qd};
				// Skip sites where the call is AA or TT and the reference base is A or T respectively (unless all sites option is given)
				let skip = !all_positions && !GT_FLAG[call.max_gt as usize][call.ref_base as usize];
				let bcf_rec = &mut self.bcf_rec;
				let mut ref_context = Vec::with_capacity(5);
				let mut called_gt = Vec::with_capacity(5);
				for call_entry in self.call_buf.iter() {
					let (ref_base, gt) = match call_entry {
						CallEntry::Call(call) => (call.ref_base, call.max_gt + 1),
						CallEntry::Skip(c) | CallEntry::Starting(c) => (*c, 0),
					};
					ref_context.push(PBASE.as_bytes()[ref_base as usize]);
					called_gt.push(gt as usize);		
				}
				let called_context: Vec<u8> = called_gt.iter().copied().map(|g| IUPAC.as_bytes()[g]).collect(); 
				let filter = if !skip {
					let tvec = &mut self.tvec;
					let rid = bs_cfg.ctg_vcf_id(self.sam_tid).expect("Contig not in VCF list");
					bcf_rec.clear();
					bcf_rec.set_rid(rid); 
					bcf_rec.set_pos(self.curr_x);
					let flt = write_fixed_columns(call, filter_ids, tvec, &call_stats, &ref_context, bcf_rec)?;
					let cpg = CPG_DISPLAY[cmp::max(CPG_STATE[called_gt[1]][called_gt[2]], CPG_STATE[called_gt[2]][called_gt[3]])];
					write_format_columns(call, filter_ids, &called_context, cpg, flt, tvec, &call_stats, bcf_rec)?;
					bcf_rec.write(&mut vcf_output.file, &mut vcf_output.hdr)?;			
					flt
				} else { 0 };
				self.curr_x += 1;
			},	
			CallEntry::Skip(_) => self.curr_x += 1,
			CallEntry::Starting(_) => (),
		}
		Ok(())
	}
}

pub fn write_vcf_entry(bs_cfg: Arc<BsCallConfig>, rx: mpsc::Receiver<WriteVcfJob>, mut bs_files: BsCallFiles, stat_tx: mpsc::Sender<StatJob>) {
	info!("write_vcf_thread starting up");
	let mut vcf_output = bs_files.vcf_output.take().unwrap();
	let filter_ids = get_filter_ids(&vcf_output.hdr);
	let mut write_state: Option<WriteState> = None;
	loop {
		match rx.recv() {
			Ok(WriteVcfJob::Quit) => {
				if let Some(ws) = write_state.as_mut() { 
					if let Err(e) = ws.finish_block(&bs_cfg, &mut vcf_output, &filter_ids, &stat_tx) { error!("finish_block failed with error: {}", e); }
				}
				break;
			},
			Ok(WriteVcfJob::CallBlock(block)) => {
				debug!("Received new call block: {}:{}", block.sam_tid, block.start);
				if let Some(ws) = write_state.as_mut() { 
					if let Err(e) = ws.finish_block(&bs_cfg, &mut vcf_output, &filter_ids, &stat_tx) {
						error!("finish_block failed with error: {}", e);
						break;
					}
				}
				write_state = Some(WriteState::new_block(block));
			},
			Ok(WriteVcfJob::GenotypeCall(call_vec)) => {
				match write_state.as_mut() {
					Some(
						ws) => {
						if let Err(e) = ws.handle_calls(call_vec, &bs_cfg, &mut vcf_output, &filter_ids, &stat_tx) {
							error!("handle_calls failed with error: {}", e);
							break;						
						}
					},
					None => panic!("Unexpected message"),
				}
			},
			Err(e) => {
				warn!("write_vcf thread recieved error: {}", e);
				break
			}
		}
	}
	info!("write_vcf thread shutting down");	
}


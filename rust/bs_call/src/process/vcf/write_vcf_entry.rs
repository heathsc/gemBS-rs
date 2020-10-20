use std::sync::{Arc, mpsc};
use std::{cmp, io};
use std::collections::VecDeque;

use crate::config::{BsCallConfig, BsCallFiles};
use crate::htslib::*;
use libc::c_int;
use crate::stats::StatJob;
use crate::process::call_genotypes::{CallBlock, GenotypeCall, CallEntry};

pub enum WriteVcfJob {
	CallBlock(CallBlock),
	GenotypeCall(Vec<CallEntry>),
	Quit,
}

const LN_10: f64 = 2.30258509299404568402;

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

const CPG_DISPLAY: [&str; 5] = [ ".", "?", "N", "H", "CG"];

fn ln_prob_2_phred(p: f64) -> usize {
	let z = (p * LN_10).exp();
	if z >= 1.0 { 255 } 
	else { cmp::min((-10.0 * (1.0 - z).ln() / LN_10).round() as usize, 255) }	
}

fn handle_filters(call: &GenotypeCall, hdr: &VcfHeader, phred: usize, fs: usize, qd: usize) -> (usize, usize) {
	let flt = if phred < 20 { 1 } else { 0 } 
		| if qd < 2 { 2 } else { 0 }
		| if fs > 60 { 4 } else { 0 }
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
			(128, hdr.id2int(BCF_DT_ID as usize, "mac1").expect("mac1 filter not found in VCF header") as usize)
		} else { (0, 0) }
	} else { (flt, hdr.id2int(BCF_DT_ID as usize, "fail").expect("fail filter not found in VCF header") as usize) }	
}

fn write_fixed_columns(call: &GenotypeCall, hdr: &VcfHeader, dp1: usize, ref_context: &[u8], bcf_rec: &mut BcfRec) -> io::Result<usize> {
	let max_gt = call.max_gt as usize;
	// Phred score for genotype call
	let phred = ln_prob_2_phred(call.gt_ll[max_gt]);
	// Alternate alleles
	let alt_alleles = REF_ALT[max_gt][call.ref_base as usize];
	// Fisher strand and quality by depth stats
	let fs = (call.fisher_strand * -10.0 + 0.5).round() as usize;
	let qd = if dp1 > 0 { phred / dp1 } else { phred };
	let mut kstr = bcf_rec.shared().expect("Couldn't get shared string from BCF rec");
	bcf_rec.set_n_sample(1);
	
	// ID - for the moment leave blank
	kstr.bcf_enc_size(0, BCF_BT_CHAR);
	
	// REF allele
	kstr.bcf_enc_size(1, BCF_BT_CHAR);
	kstr.putc(ref_context[2]);
	bcf_rec.set_rlen(1);
		
	// ALT alleles
	for allele in alt_alleles.as_bytes().iter().copied() {
		kstr.bcf_enc_size(1, BCF_BT_CHAR);
		kstr.putc(allele);
	}
	bcf_rec.set_n_allele(1 + alt_alleles.len());

	// QUAL
	bcf_rec.set_qual(phred as f32);
	
	// FILTER
	let (filter, filter_id) = handle_filters(call, hdr, phred, fs, qd);	
	kstr.bcf_enc_int1(filter_id as c_int);
	
	// INFO
	bcf_rec.set_n_info(1);
	kstr.bcf_enc_int1(hdr.id2int(BCF_DT_ID as usize, "CX").expect("CX ID not found in VCF header") as c_int);
	kstr.bcf_enc_vchar(ref_context);	
	Ok(filter)
}

fn write_format_columns(hdr: &VcfHeader, called_context: &[u8], cpg: &str, filter: usize, bcf_rec: &mut BcfRec) -> io::Result<()> {
	
	Ok(())
}

/*
		bcf->n_fmt = 11;
		// Handle sample fields
		kstring_t *str = &bcf->indiv;
		// GT
		int32_t x[8];
		uint8_t gg = gt_int[gt][rfix];
		x[0] = (gg >> 4);
		x[1] = (gg & 0xf);
		bcf_enc_int1(str, par->work.vcf_ids[VCF_FMT_GT]);
		bcf_enc_vint(str, 2, x, 2);
		// FT
		char fbuf[24];
		int flen = 0;
		if(flt & 15) {
			char *p = fbuf;
			int f_ix = 0;
			uint32_t flt1 = flt & 31;
			bool first = true;
			while(flt1) {
				if(flt1 & 1) {
					if(!first) *p++ = ';';
					const char *p1 = par->defs.flt_name[f_ix];
					while((*p++ = *p1++));
					first = false;
				}
				flt1 >>= 1;
				f_ix++;
			}
			*p = 0;
			flen = p - fbuf;
		} else {
			strcpy(fbuf, "PASS");
			flen = 4;
		}
		bcf_enc_int1(str, par->work.vcf_ids[VCF_FMT_FT]);
		bcf_enc_size(str, flen, BCF_BT_CHAR);
		kputsn_(fbuf, flen, str);

*/

struct WriteState {
	sam_tid: usize,
	curr_x: usize,
	call_buf: VecDeque<CallEntry>,
	bcf_rec: BcfRec,
}

impl WriteState {
	fn new_block(call_block: CallBlock) -> Self {
		let mut v = VecDeque::with_capacity(5);
		for _ in 0..3 { v.push_back(CallEntry::Skip(0)) }
		for c in call_block.prec_ref_bases.iter() { v.push_back(CallEntry::Skip(*c)) }
		let bcf_rec = BcfRec::new().expect("Couldn't allocate Bcf Record");
		Self { sam_tid: call_block.sam_tid, curr_x: call_block.start, call_buf: v, bcf_rec}
	}
	fn finish_block(&mut self, bs_cfg: &BsCallConfig, vcf_output: &VcfFile, stat_tx: &mpsc::Sender<StatJob>) -> io::Result<()> {
		for _ in 0..2 {
			self.add_entry(CallEntry::Skip(0));
			self.write_entry(bs_cfg, vcf_output, stat_tx)?;
		}
		Ok(())
	}
	fn handle_calls(&mut self, mut call_vec: Vec<CallEntry>, bs_cfg: &BsCallConfig, vcf_output: &VcfFile, stat_tx: &mpsc::Sender<StatJob>) -> io::Result<()> {
		for entry in call_vec.drain(..) {
			self.add_entry(entry);
			self.write_entry(bs_cfg, vcf_output, stat_tx)?;
		}
		Ok(())
	}
	fn add_entry(&mut self, entry: CallEntry) {
		assert!(self.call_buf.len() == 5);
		self.call_buf.push_back(entry);
		let _ = self.call_buf.pop_front();		
	}
	fn write_entry(&mut self, bs_cfg: &BsCallConfig, vcf_output: &VcfFile, stat_tx: &mpsc::Sender<StatJob>) -> io::Result<()> {
		if let CallEntry::Call(call) = &self.call_buf[2] { 
			let all_positions = bs_cfg.conf_hash.get_bool("all_positions");
			let dp1: usize = call.counts[0..4].iter().sum();
			let d_inf: usize = call.counts[4..].iter().sum();
			// Skip sites with no coverage
			if dp1 + d_inf == 0 { return Ok(()) }
			// Skip sites where the call is AA or TT and the reference base is A or T respectively (unless all sites option is given)
			let skip = !all_positions && !GT_FLAG[call.max_gt as usize][call.ref_base as usize];
			let bcf_rec = &mut self.bcf_rec;
			let mut ref_context = Vec::with_capacity(5);
			let mut called_gt = Vec::with_capacity(5);
			for call_entry in self.call_buf.iter() {
				let (ref_base, gt) = match call_entry {
					CallEntry::Call(call) => (call.ref_base, call.max_gt + 1),
					CallEntry::Skip(c) => (*c, 0),
				};
				ref_context.push(PBASE.as_bytes()[ref_base as usize]);
				called_gt.push(gt as usize);		
			}
			let called_context: Vec<u8> = called_gt.iter().copied().map(|g| IUPAC.as_bytes()[g]).collect(); 
			let filter = if !skip {
				let rid = bs_cfg.ctg_vcf_id(self.sam_tid).expect("Contig not in VCF list");
				bcf_rec.clear();
				bcf_rec.set_rid(rid); 
				bcf_rec.set_pos(self.curr_x);		
				let flt = write_fixed_columns(call, &vcf_output.hdr, dp1, &ref_context, bcf_rec)?;
				let cpg = CPG_DISPLAY[cmp::max(CPG_STATE[called_gt[1]][called_gt[2]], CPG_STATE[called_gt[2]][called_gt[3]])];
				write_format_columns(&vcf_output.hdr, &called_context, cpg, flt, bcf_rec)?;
				flt
			} else { 0 };
			
		}
		self.curr_x += 1;
		Ok(())
	}
}

pub fn write_vcf_entry(bs_cfg: Arc<BsCallConfig>, rx: mpsc::Receiver<WriteVcfJob>, mut bs_files: BsCallFiles, stat_tx: mpsc::Sender<StatJob>) {
	info!("write_vcf_thread starting up");
	let vcf_output = bs_files.vcf_output.take().unwrap();
	let mut write_state: Option<WriteState> = None;
	loop {
		match rx.recv() {
			Ok(WriteVcfJob::Quit) => {
				if let Some(ws) = write_state.as_mut() { 
					if let Err(e) = ws.finish_block(&bs_cfg, &vcf_output, &stat_tx) { error!("finish_block failed with error: {}", e); }
				}
				break;
			},
			Ok(WriteVcfJob::CallBlock(block)) => {
				debug!("Received new call block: {}:{}", block.sam_tid, block.start);
				if let Some(ws) = write_state.as_mut() { 
					if let Err(e) = ws.finish_block(&bs_cfg, &vcf_output, &stat_tx) {
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
						if let Err(e) = ws.handle_calls(call_vec, &bs_cfg, &vcf_output, &stat_tx) {
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


use std::sync::{Arc, mpsc};
use std::collections::HashMap;

use crate::config::BsCallConfig;
use crate::stats::*;
use crate::process::vcf::*;
use crate::process::pileup::GC_BIN_SIZE;
use crate::process::call_genotypes::fisher::FisherTest;
use crate::rusage::*;

const GT_HET: [bool; 10] = [false, true, true, true, false, true, true, false, true, false];

const MUT_AC: usize = 0;
const MUT_AG: usize = 1;
const MUT_AT: usize = 2;
const MUT_CA: usize = 3;
const MUT_CG: usize = 4;
const MUT_CT: usize = 5;
const MUT_GA: usize = 6;
const MUT_GC: usize = 7;
const MUT_GT: usize = 8;
const MUT_TA: usize = 9;
const MUT_TC: usize = 10;
const MUT_TG: usize = 11;
const MUT_NO: usize = 12;

pub const MUT_NAMES:[&str; 12] = [ "A>C", "A>G", "A>T", "C>A", "C>G", "C>T", "G>A", "G>C", "G>T", "T>A", "T>C", "T>G" ];

const MUT_TYPE: [[usize; 5]; 10] = [
	[MUT_NO, MUT_NO, MUT_CA, MUT_GA, MUT_TA],   // AA
	[MUT_NO, MUT_AC, MUT_CA, MUT_NO, MUT_NO],   // AC
	[MUT_NO, MUT_AG, MUT_NO, MUT_GA, MUT_NO],   // AG
	[MUT_NO, MUT_AT, MUT_NO, MUT_NO, MUT_TA],   // AT
	[MUT_NO, MUT_AC, MUT_NO, MUT_GC, MUT_TC],   // CC
	[MUT_NO, MUT_NO, MUT_CG, MUT_GC, MUT_NO],   // CG
	[MUT_NO, MUT_NO, MUT_CT, MUT_NO, MUT_TC],   // CT
	[MUT_NO, MUT_AG, MUT_CG, MUT_NO, MUT_TG],   // GG
	[MUT_NO, MUT_NO, MUT_NO, MUT_GT, MUT_TG],   // GT
	[MUT_NO, MUT_AT, MUT_CT, MUT_GT, MUT_NO]    // TT
];

pub const SITE_TYPE_ALL: usize = 0;
pub const SITE_TYPE_VARIANT: usize = 1;
pub const SITE_TYPE_CPG_REF: usize = 2;
pub const SITE_TYPE_CPG_NON_REF: usize = 3;

struct BinDist {
	ln_p: Vec<(f64, f64)>,
	dist: Vec<f64>,
	ft: FisherTest,
}

impl BinDist {
	fn new(n: usize) -> Self {
		let ln_p: Vec<_> = (0..n-1).map(|x| {
			let dn = n as f64;
			((((x + 1) as f64) / dn).ln(), (((n - 1 - x) as f64) / dn).ln())
		}).collect();
		let dist = vec!(0.0; n + 1);
		Self{ln_p, dist, ft: FisherTest::new()}
	}
	fn dist(&mut self, a: usize, b: usize) -> &[f64] {
		assert!(a + b > 0);
		let d = &mut self.dist;
		let konst = self.ft.lfact(a + b + 1) - self.ft.lfact(a) - self.ft.lfact(b);
		d.clear();
		let (mut sum, last) = match(a, b) {
			(_, 0) =>  {
				d.push(0.0);
				(konst.exp(), konst.exp())
			},
			(0, _) => {
				d.push(konst.exp());
				(d[0], 0.0)
			},
			_ => {
				d.push(0.0);
				(0.0, 0.0)
			}
		};
		let (da, db) = ((a as f64), (b as f64));
		for (lp, lp1) in self.ln_p.iter().copied() {
			let z = (konst + da * lp + db * lp1).exp();
			d.push(z);
			sum += z;
		}
		d.push(last);
		d.iter_mut().for_each(|z| *z /= sum);
		d
	} 
	
}
pub struct CovStats {
	pub var: usize,
	pub cpg: [usize; 2],
	pub cpg_inf: [usize; 2],
	pub all: usize,
	pub gc_pcent: [usize; GC_BIN_SIZE as usize],
}

impl CovStats { 
	fn new() -> Self { Self{var: 0, all: 0, cpg: [0; 2], cpg_inf: [0; 2], gc_pcent: [0; GC_BIN_SIZE as usize] }}
}

#[derive(Default)]
pub struct VCFBasic { 
	pub snps: Counts,
	pub indels: Counts,
	pub multiallelic: Counts,
	pub ref_cpg: Counts,
	pub non_ref_cpg: Counts,	
	pub dbsnp_sites: Counts,
	pub dbsnp_variants: Counts,
}

pub struct VcfStats {
	pub total_stats: VCFBasic,
	pub contig_stats: HashMap<String, VCFBasic>,
	pub cov_stats: HashMap<usize, CovStats>,
	pub mut_counts: [Counts; 12],		
	pub dbsnp_mut_counts: [Counts; 12],
	pub qual: [[usize; 256]; 4],
	pub filter_counts: [[usize; 2]; 32],
	pub cpg_ref_meth: [[f64; 2]; 101],		
	pub cpg_non_ref_meth: [[f64; 2]; 101],	
	pub fs_stats: HashMap<usize, [usize; 2]>,	
	pub qd_stats: HashMap<usize, [usize; 2]>,	
	pub mq_stats: HashMap<usize, [usize; 2]>,	
}

impl VcfStats {
	fn new() -> Self {
		Self {
			total_stats: VCFBasic::default(),
			contig_stats: HashMap::new(), cov_stats: HashMap::new(),
			fs_stats: HashMap::new(), qd_stats: HashMap::new(), mq_stats: HashMap::new(),
			mut_counts: Default::default(), dbsnp_mut_counts: Default::default(),
			qual: [[0; 256]; 4], filter_counts: Default::default(),
			cpg_ref_meth: [[0.0; 2]; 101], cpg_non_ref_meth: [[0.0; 2]; 101]
		}
	}
}

const BS_SNPS: usize = 1;
const BS_INDELS: usize = 2;
const BS_MULTI: usize = 4;
const BS_REF_CPG: usize = 8;
const BS_NON_REF_CPG: usize = 16;

fn get_basic_stats(cs: &CallStats) -> usize { 
	(if (cs.flags & CALL_STATS_MULTI) != 0 { BS_MULTI }
	else if (cs.flags & CALL_STATS_SNP) != 0 { BS_SNPS }
	else { 0 }) 
	| (if (cs.cpg_status & 7) == 4 { // Homozygous CPG
		if (cs.cpg_status & CPG_STATUS_REF_CPG) != 0 { BS_REF_CPG } else { BS_NON_REF_CPG } 
	} else { 0 })
}

fn add_basic_stats(bs: &mut VCFBasic, filter: u8, rs_found: bool, flags: usize) {
	let cts = if filter == 0 { Counts::make(1, 1) } else { Counts::make(1, 0) };
	if (flags & BS_SNPS) != 0 { bs.snps += cts }
	else if (flags & BS_INDELS) != 0 { bs.indels += cts }
	else if (flags & BS_MULTI) != 0 { bs.multiallelic += cts }
	if (flags & BS_REF_CPG) != 0 { bs.ref_cpg += cts }
	else if (flags & BS_NON_REF_CPG) != 0 { bs.non_ref_cpg += cts }
	if rs_found {
		bs.dbsnp_sites += cts;
		if (flags & (BS_SNPS | BS_MULTI | BS_INDELS)) != 0 { bs.dbsnp_variants += cts }
	}
}

fn handle_meth_stats(vs: &mut VcfStats, bin_dist: &mut BinDist, a: usize, b: usize, ref_cpg: bool, cs: &CallStats) {
	let gcov = vs.cov_stats.entry(cs.d_inf as usize).or_insert_with(CovStats::new);
	gcov.cpg_inf[if ref_cpg { 0 } else { 1 }] += 1;
	vs.qual[if ref_cpg { SITE_TYPE_CPG_REF } else {SITE_TYPE_CPG_NON_REF}][cs.phred as usize] += 1;
	if a + b > 0 {
		let d = bin_dist.dist(a, b);
		let z = if cs.filter == 0 { 1.0 } else { 0.0 };
		let m_ref = if ref_cpg { &mut vs.cpg_ref_meth } else { &mut vs.cpg_non_ref_meth };
		for (i, p) in d.iter().enumerate() { 
			m_ref[i][0] += *p;
			m_ref[i][1] += *p * z; 
		}
	}
}

fn add_filter_counts(vcf_stats: &mut VcfStats, cs: &CallStats) {
	let het = if GT_HET[cs.gt as usize] { 1 } else { 0 };
	vcf_stats.qd_stats.entry(cs.qd as usize).or_insert([0, 0])[het] += 1;	
	vcf_stats.fs_stats.entry(cs.fs as usize).or_insert([0, 0])[het] += 1;	
	vcf_stats.mq_stats.entry(cs.mq as usize).or_insert([0, 0])[het] += 1;	
	vcf_stats.filter_counts[(cs.filter & 31) as usize][het] += 1;
	vcf_stats.qual[SITE_TYPE_ALL][cs.phred as usize] += 1;
}

fn mutation_stats(vs: &mut VcfStats, cs: &CallStats) {
	let mut_type = MUT_TYPE[cs.gt as usize][cs.ref_base as usize];
	if mut_type != MUT_NO {
		let cts = if cs.filter == 0 { Counts::make(1, 1) } else { Counts::make(1, 0) };
		vs.mut_counts[mut_type] += cts;
		if (cs.flags & CALL_STATS_RS_FOUND) != 0 { vs.dbsnp_mut_counts[mut_type] += cts }
	}	
}

fn handle_stats(call_stats: &[CallStats], vcf_stats: &mut VcfStats, bin_dist: &mut BinDist, bs_cfg: &BsCallConfig) {
	let cname = bs_cfg.ctg_name(call_stats[0].sam_tid).to_owned();
	let mut ctg_stats = vcf_stats.contig_stats.entry(cname).or_insert_with(VCFBasic::default);
	for cs in call_stats.iter() {
		let flags = get_basic_stats(cs);
		let dp = (cs.d_inf + cs.dp1) as usize;
		let gcov = vcf_stats.cov_stats.entry(dp).or_insert_with(CovStats::new);
		gcov.all += 1;
		if cs.gc != 255 { gcov.gc_pcent[cs.gc as usize] += 1 };
		if (cs.cpg_status & 7) == 4 { gcov.cpg[if (cs.cpg_status & CPG_STATUS_REF_CPG) != 0 { 0 } else { 1 }] += 1 }
		if (cs.flags & CALL_STATS_SKIP) != 0 { continue; }
		let rs_found = (cs.flags & CALL_STATS_RS_FOUND) != 0;
		add_basic_stats(&mut ctg_stats, cs.filter, rs_found, flags);
		add_basic_stats(&mut vcf_stats.total_stats, cs.filter, rs_found, flags);
		if (flags & (BS_SNPS | BS_INDELS | BS_MULTI)) != 0 {
			vcf_stats.qual[SITE_TYPE_VARIANT][cs.phred as usize] += 1;
			gcov.var += 1;
		}
	}
	for cs in call_stats.iter().filter(|c| (c.flags & CALL_STATS_SKIP) == 0) {
		add_filter_counts(vcf_stats, cs);
		if let Some((a, b)) = cs.meth_cts { handle_meth_stats(vcf_stats, bin_dist, a, b, (cs.cpg_status & CPG_STATUS_REF_CPG) != 0, cs) }
		mutation_stats(vcf_stats, cs);	
	}
}

pub fn collect_vcf_stats(bs_cfg: Arc<BsCallConfig>, rx: mpsc::Receiver<Option<Vec<CallStats>>>, stat_tx: mpsc::Sender<StatJob>) {
	info!("collect_vcf_stats_thread starting up");
	let mut vcf_stats = VcfStats::new();
	let mut bin_dist = BinDist::new(100);
	loop {
		match rx.recv() {
			Ok(None) => break,
			Ok(Some(call_stats)) => {
				debug!("Received new call stats block");
				if !call_stats.is_empty() {
					handle_stats(&call_stats, &mut vcf_stats, &mut bin_dist, &bs_cfg);
				}
			},
			Err(e) => {
				warn!("collect_vcf_stats thread recieved error: {}", e);
				break
			}
		}
	}
	let _ = stat_tx.send(StatJob::AddVcfStats(vcf_stats));
	if let Ok(ru_thread) = Rusage::get(RusageWho::RusageThread) {
		info!("collect_vcf_stats_thread shutting down: user {} sys {}", ru_thread.utime(), ru_thread.stime());	
	}
 }


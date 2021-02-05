use std::str::FromStr;
use std::io::{self, Write};
use std::sync::Arc;
use std::collections::HashMap;
use std::ops::Deref;

use crossbeam_channel::Receiver;

use r_htslib::{HtsFile, VcfHeader, htsThreadPool};
use libc::c_int;

use super::config::*;
use super::read_vcf::unpack::{Strand, RecordBlock, RecordBlockElem};
use super::read_vcf::BREC_BLOCK_SIZE;
use super::process::{Recv, TPool};

mod output_cpg;
use output_cpg::*;
pub mod output_noncpg;
pub use output_noncpg::*;

pub const GT_IUPAC: &[u8] = "AMRWCSYGKT".as_bytes();
pub const GT_MASK: [u8; 10] = [0x11, 0xb3, 0x55, 0x99, 0xa2, 0xf6, 0xaa, 0x54, 0xdc, 0x88];

// REC_BLOCK_SIZE can not be smaller than BREC_BLOCK_SIZE
pub const REC_BLOCK_SIZE: usize = BREC_BLOCK_SIZE;

pub struct MethRec {
	counts: [c_int; 8],	
	gt_probs: [f64; 10],
	meth: [f64; 6],
	mq: u8,
	cx: [u8; 5],
	max_gt: Option<u8>,
}

const GT_IDX: [[Option<u8>; 10]; 2] = [
	[None, Some(2), None, None, Some(0), Some(2), Some(1), None, None, None],
	[None, None, Some(1), None, None, Some(2), None, Some(0), Some(2), None],
];	

impl MethRec {
	pub fn new(counts: [c_int; 8], gt_probs: [f64; 10], meth: [f64; 6], cx: [u8; 5], mq: u8, max_gt: Option<u8>) -> Self {
		Self{counts, gt_probs, meth, cx, mq, max_gt}
	}	
	pub fn counts(&self) -> &[c_int] { &self.counts }
	pub fn gt_probs(&self) -> &[f64] { &self.gt_probs }
	pub fn gt_probs_mut(&mut self) -> &mut[f64] { &mut self.gt_probs }
	pub fn mq(&self) -> u8 { self.mq }
	pub fn cx(&self) -> &[u8] { &self.cx }
	pub fn max_gt(&self) -> Option<u8> { self.max_gt }	
	pub fn set_max_gt(&mut self, g: u8) { self.max_gt = Some(g) }	
	pub fn get_meth(&self, strand: Strand) -> Option<f64> {
		if let Some(gt) = self.max_gt {
			let (v, m) = if matches!(strand, Strand::C) { (&GT_IDX[0], &self.meth[..3]) } else { (&GT_IDX[1], &self.meth[3..]) };
			v[gt as usize].map(|i| m[i as usize])
		} else { None }
	}
}

pub struct Record {
	rid: u32,
	pos: u32,
	cx: [u8; 5],
	gt_strand: Option<(u8, Strand)>,
}

impl Record {
	pub fn new(rid: u32, pos: u32, cx: [u8; 5], gt_strand: Option<(u8, Strand)>) -> Self {
		Self{rid, pos, cx, gt_strand}
	}	
	pub fn cx(&self) -> &[u8] { &self.cx }
	pub fn rid(&self) -> u32 { self.rid }
	pub fn pos(&self) -> u32 { self.pos }
	pub fn gt(&self) -> Option<u8> { self.gt_strand.map(|(gt, _)| gt) }
	pub fn strand(&self) -> Option<Strand> {
		self.gt_strand.map(|(_, s)|
			match s {
				Strand::Amb | Strand::Unk => {
					if self.cx[2].to_ascii_uppercase() == b'G' { Strand::G } else { Strand::C } 					
				},
				_ => s,
			} 
		)
	}
}

pub fn calc_phred(z: f64) -> u8 {
	if z <= 0.0 { 255 } else { ((-10.0 * z.log10()) as usize).min(255) as u8 }	
}

pub struct OutputOpts {
	min_inform: usize,
	min_n: usize,
	min_nc: usize,
	mode: Mode,
	bw_mode: Mode,
	select: Select,	
	threshold: u8,
}

impl OutputOpts {
	pub fn new(chash: &ConfHash) -> Self {
		Self {
			min_inform: chash.get_int("inform"),
			min_nc: chash.get_int("min_nc"),
			min_n: chash.get_int("number"),
			mode: chash.get_mode("mode"),
			bw_mode: chash.get_mode("bw_mode"),
			select: chash.get_select("select"),
			threshold: chash.get_int("threshold") as u8
		}
	}
	pub fn min_inform(&self) -> c_int { self.min_inform as c_int }
	pub fn min_n(&self) -> c_int { self.min_n as c_int }
	pub fn min_nc(&self) -> c_int { self.min_nc as c_int }
	pub fn mode(&self) -> Mode { self.mode }
	pub fn bw_mode(&self) -> Mode { self.bw_mode }
	pub fn select(&self) -> Select { self.select }
	pub fn threshold(&self) -> u8 { self.threshold }
}

fn open_output_file(name: &str, chash: &ConfHash, tp: TPool) -> HtsFile {
	let mut fname = String::from_str(name).unwrap();
	let compress = chash.get_bool("compress");
	let output_mode = if compress { 
		if !fname.ends_with(".gz") { fname.push_str(".gz")}
		"wz" 
	} else { "w" };
	match HtsFile::new(&fname, output_mode) {
		Ok(mut f) => {
			if let Some(tpool) = tp.deref() { f.set_thread_pool(tpool); }
			f
		},
		Err(e) => panic!("Couldn't open nonCpG file {} for output: {}", fname, e),
	}
}

pub fn get_prob_dist(prb: &mut [f64]) {
	assert!(!prb.is_empty());
	let ns = prb.len();
	let mut x = 1.0;
	for ix in 0..ns {
		let z = prb[ix];
		prb[ix] = x;
		let t = prb[0];
		prb[0] *=  1.0 - z;
		let mut y = t;
		for p in &mut prb[1..=ix] {
			let t = *p; 
			*p = y * z + t * (1.0 - z);
			y = t;
		}
		x *= z;
	} 
} 

type PrintHeader = fn(&mut HtsFile, &VcfHeader) -> io::Result<()>;
type OutputBlock = fn(&mut HtsFile, &RecordBlock, Option<RecordBlockElem>, &ConfHash, &VcfHeader) -> io::Result<()>;

// Print header for CpG and NonCpG tab separated variable files
fn print_tsv_header(f: &mut HtsFile, hdr: &VcfHeader) -> io::Result<()> {
	write!(f, "Contig\tPos0\tPos1\tRef")?;
	for i in 0..hdr.nsamples() {
		let name = hdr.sample_name(i)?;
		write!(f, "\t{}:Call\t{}:Flags\t{}:Meth\t{}:non_conv\t{}:conv\t{}:support_call\t{}:total", name, name, name, name, name, name, name)?;
	}	
	writeln!(f)
}

pub fn output_handler(chash: &ConfHash, hdr: &VcfHeader, r: Recv, oname: &str, ph: PrintHeader, ob: OutputBlock, tp: TPool) {
	let mut outfile = open_output_file(oname, chash, tp);
	if !chash.get_bool("no_header") { ph(&mut outfile, hdr).expect("Error writing header") }
	let mut blk_store: HashMap<usize, Arc<RecordBlock>> = HashMap::new();
	let mut curr_ix = 0;	
	let mut pblk: Option<Arc<RecordBlock>> = None;
	for (ix, rblk) in r.iter() {
		if ix == curr_ix {
			let prev = if let Some(b) = pblk.as_ref() { b.last() } else { None };
			ob(&mut outfile, &rblk, prev, chash, hdr).expect("Error writing file");
			pblk = Some(rblk.clone());
			curr_ix += 1;
			while let Some(blk) = blk_store.remove(&curr_ix) {
				let prev = if let Some(b) = pblk.as_ref() { b.last() } else { None };
				ob(&mut outfile, &blk, prev, chash, hdr).expect("Error writing file");
				pblk = Some(rblk.clone());
				curr_ix += 1;
			}
		} else { blk_store.insert(ix, rblk); }
	}
	if !blk_store.is_empty() { warn!("Blocks left over in output_noncpg_thread") }
}

pub fn output_cpg_thread(chash: Arc<ConfHash>, hdr: Arc<VcfHeader>, r: Recv, tp: TPool) {
	let output = chash.get_str("cpgfile").expect("CpG output filename is missing");
	debug!("output_cpg_thread starting up");
	output_handler(&chash, &hdr, r, output, print_tsv_header, output_cpg, tp);
	debug!("output_cpg_thread closing down")
}

pub fn output_noncpg_thread(chash: Arc<ConfHash>, hdr: Arc<VcfHeader>, r: Recv, tp: TPool) {
	let output = chash.get_str("noncpgfile").expect("Non CpG output filename is missing");
	debug!("output_noncpg_thread starting up");
	output_handler(&chash, &hdr, r, output, print_tsv_header, output_noncpg, tp);
	debug!("output_noncpg_thread closing down")
}

pub fn output_bed_methyl_thread(chash: Arc<ConfHash>, hdr: Arc<VcfHeader>, r: Recv, tp: TPool) {
	for msg in r.iter() {
		// Do nothing
	}
	info!("output_bed_methyl_thread closing down")	
}
use std::sync::Arc;
use std::convert::TryInto;
use std::f64::consts::LN_10;

use libc::c_int;
use crossbeam_channel::{Sender, Receiver};
use r_htslib::*;
use crate::config::*;

use super::model::{Model, MAX_QUAL};
use super::BrecBlock;
use crate::output::{Record, MethRec, REC_BLOCK_SIZE};

pub enum RecordBlockElem<'a> {
	Single((&'a Record, &'a MethRec)),
	Multi((&'a Record, &'a [MethRec])),
}

impl <'a>RecordBlockElem<'a> {
	pub fn record(&'a self) -> &'a Record {
		match self {
			RecordBlockElem::Single((r, _)) => r,
			RecordBlockElem::Multi((r, _)) => r,
		}
	} 
}

pub enum RecordBlock {
	Single(Vec<(Record, MethRec)>),
	Multi(Vec<(Record, Box<[MethRec]>)>),
}

impl RecordBlock {
	fn len(&self) -> usize {
		match self {
			RecordBlock::Single(v) => v.len(),
			RecordBlock::Multi(v) => v.len(),
		}
	}
	pub fn last(&self) -> Option<RecordBlockElem> {
		match self {
			RecordBlock::Single(v) => v.last().map(|(r, m)| RecordBlockElem::Single((r, m))),
			RecordBlock::Multi(v) => v.last().map(|(r, mv)| RecordBlockElem::Multi((r, mv))),
		}		
	}
}
const BASE_MAP: [u8; 256] = [
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

fn handle_aq(aq: &[i32], ct: &[c_int]) -> [c_int; 8] {
	if aq.len() != 8 {
		let mut tq = [0; 8];
		let mut k = 0;
		for (i, c) in ct.iter().enumerate() {
			if *c > 0 { 
				tq[i] = aq[k] as c_int;
				k += 1;
				if k == aq.len() { break }
			}
		}
		tq
	} else { aq.try_into().unwrap() }
}

fn setup_model(cf: &ConfHash) -> Model {
	Model::new(
		(cf.get_float("under_conversion"), cf.get_float("over_conversion")),
		cf.get_float("reference_bias"),
		cf.get_bool("haploid"), 
		false // Get natural logs
	)
}


#[derive(Clone, Copy)]
pub enum Strand { C, G, Amb, Unk }

const GT_TYPE: [Strand; 10] = [ 
	Strand::Unk,  // AA
	Strand::C,    // AC
	Strand::G,    // AG
	Strand::Unk,  // AT
	Strand::C,    // CC
	Strand::Amb,  // CG
	Strand::C,    // CT
	Strand::G,    // GG
	Strand::G,    // GT
	Strand::Unk,  // TT
];

fn find_strand(v: &[f64], mx: usize) -> Strand {
	match GT_TYPE[mx] {
		Strand::Unk => {
			let (cmx, _) = v[2..].iter().cloned().enumerate().filter(|(i, _)| !matches!(GT_TYPE[i + 2], Strand::Unk))
				.fold((1, v[1]), |(i, m), (j, l)| if l > m { (j + 2, l) } else { (i, m) });
			GT_TYPE[cmx]
		},
		_ => GT_TYPE[mx],
	}
} 

fn calc_common_gt(recv: &mut [MethRec], common_gt: bool) -> Option<(u8, Strand)> {
	if recv.iter().any(|r| r.max_gt().is_some()) {
		// Sum genotype probabilities over non-skipped samples
		let mut ll = [0.0; 10];
		let mut n = 0;
		for rec in recv.iter().filter(|r| r.max_gt().is_some()) {
			n += 1;
			rec.gt_probs().iter().zip(ll.iter_mut()).for_each(|(x,y)| *y += *x); 
		}
		if n > 1 {
			// If more than 1 non-skipped samples, re-scale ll
			let (mx, max) = ll[1..].iter().cloned().enumerate().fold((0, ll[0]), |(i, m), (j, l)| if l > m { (j + 1, l) } else { (i, m) });
			let sum = (ll.iter().cloned().fold(0.0, |s, x| s + (x - max).exp())).ln();
			ll.iter_mut().for_each(|x| *x = (*x - max - sum) / LN_10);
			// and update all non-skipped recs if required
			if common_gt { 
				for rec in recv.iter_mut().filter(|r| r.max_gt().is_some()) {
					rec.gt_probs_mut().iter_mut().zip(ll.iter()).for_each(|(x,y)| *x = *y);
					rec.set_max_gt(mx as u8); 
				}	
			}
			// Find max C/G genotype
			Some((mx as u8, find_strand(&ll, mx)))
		} else { 
			let r = recv.iter().find(|r| r.max_gt().is_some()).unwrap();
			let mx = r.max_gt().unwrap();
			Some((mx, find_strand(r.gt_probs(), mx as usize)))
		}
	} else { None }
}

pub struct UnpackData {
	mdb_mc8: MallocDataBlock::<i32>,
	mdb_aq: MallocDataBlock::<i32>,
	mdb_mq: MallocDataBlock::<i32>,
	mdb_cx: MallocDataBlock::<u8>,
	model: Model,
	bq: c_int,
	common_gt: bool,
	ns: usize,
	idx: usize,
	mrec_vec: Option<Vec<MethRec>>,
	rec_blk: Option<RecordBlock>,
}

impl UnpackData {
	pub fn new(chash: &ConfHash, ns: usize) -> Self {
		let mdb_mc8 = MallocDataBlock::<i32>::new();
		let mdb_aq = MallocDataBlock::<i32>::new();
		let mdb_mq = MallocDataBlock::<i32>::new();
		let mdb_cx = MallocDataBlock::<u8>::new();
		let model = setup_model(&chash);
		let bq = chash.get_int("bq_threshold").min(MAX_QUAL) as c_int;
		let common_gt = chash.get_bool("common_gt");
		let mrec_vec = Some(Vec::with_capacity(ns));
		let rec_blk = Some(if ns > 1 { RecordBlock::Multi(Vec::with_capacity(REC_BLOCK_SIZE))}
		else { RecordBlock::Single(Vec::with_capacity(REC_BLOCK_SIZE))});
		Self{ mdb_mc8, mdb_aq, mdb_mq, mdb_cx, model, bq, common_gt, ns, mrec_vec, rec_blk, idx: 0}		
	}	
}

pub fn send_blk(udata: &mut UnpackData, channel_vec: &[Sender<(usize, Arc<RecordBlock>)>]) {
	if udata.rec_blk.as_ref().unwrap().len() > 0 {
		let arc_rec_blk = Arc::new(udata.rec_blk.take().unwrap());
		for s in channel_vec.iter() {
			let rb = arc_rec_blk.clone();
			s.send((udata.idx, rb)).expect("Error sending record block");
		} 
		drop(arc_rec_blk);
		udata.rec_blk = Some(if udata.ns > 1 { RecordBlock::Multi(Vec::with_capacity(REC_BLOCK_SIZE))}
		else { RecordBlock::Single(Vec::with_capacity(REC_BLOCK_SIZE))});
	}
}

pub fn unpack_vcf(brec: &mut BcfRec, idx: usize, hdr: &VcfHeader, udata: &mut UnpackData, channel_vec: &[Sender<(usize, Arc<RecordBlock>)>]) {
	let alls = brec.alleles();
	// We only consider sites where at least one allele is C or G 
	if !alls.iter().any(|a| a == &"C" || a == &"G") { return }
	// Get site context from INFO field
	if brec.get_info_u8(&hdr, "CX", &mut udata.mdb_cx).is_none() || udata.mdb_cx.len() != 5 { return }
	if udata.idx != idx {
		send_blk(udata, channel_vec);
		udata.idx = idx;
	}
	let ns = udata.ns;
	let cx: [u8; 5] = (&udata.mdb_cx as &[u8]).try_into().unwrap();
	// Get reference base coded as 1,2,3,4 for A,C,G,T or 0 for anything else 
	let ref_base = BASE_MAP[cx[2] as usize];
		
	// Get format values
	if brec.get_format_i32(&hdr, "MC8", &mut udata.mdb_mc8).is_none() || udata.mdb_mc8.len() != 8 * ns
		|| brec.get_format_u8(&hdr, "CX", &mut udata.mdb_cx).is_none() || udata.mdb_cx.len() != 5 * ns
		|| brec.get_format_i32(&hdr, "MQ", &mut udata.mdb_mq).is_none() || udata.mdb_mq.len() != ns { return }
	brec.get_format_i32(&hdr, "AMQ", &mut udata.mdb_aq).or_else(|| brec.get_format_i32(&hdr, "AQ", &mut udata.mdb_aq));
		
	// Replace missing values
	udata.mdb_mc8.iter_mut().for_each(|x| if *x == bcf_int32_missing {*x = 0});
	udata.mdb_mq.iter_mut().for_each(|x| if *x == bcf_int32_missing {*x = 0});
	udata.mdb_aq.iter_mut().for_each(|x| if *x == bcf_int32_missing {*x = 0} else if *x > (MAX_QUAL as i32) { *x = MAX_QUAL as i32});
	
	let mut mrec_vec = udata.mrec_vec.as_mut().unwrap();	 
	let ne_aq = udata.mdb_aq.len() / ns;
	mrec_vec.clear();
	for ix in 0..ns {
		let cx: [u8; 5] = (&udata.mdb_cx[ix * 5..(ix + 1) * 5] as &[u8]).try_into().unwrap();
		let counts: [c_int; 8] = (&udata.mdb_mc8[ix * 8..(ix + 1) * 8] as &[c_int]).try_into().unwrap();
		let aq = if ne_aq != 0 { handle_aq(&udata.mdb_aq[ix * ne_aq..(ix + 1) * ne_aq], &counts) } 
		else { [udata.bq; 8] };			
 		let mq = udata.mdb_mq[ix] as u8;
		let mut meth = [0.0; 6];
		let (max_gt, gt_probs) = if counts.iter().any(|x| *x > 0) {
			let gt = udata.model.calc_gt_prob(&counts, &aq, ref_base, Some(&mut meth));
			(Some(gt.0 as u8), gt.1)
		} else {		
			(None, [0.0; 10])
		};
		mrec_vec.push(MethRec::new(counts, gt_probs, meth, cx, mq, max_gt));
	}
	// Get common genotype call 
	let gt_strand = if udata.ns > 1 { calc_common_gt(&mut mrec_vec, udata.common_gt) } 
	else { mrec_vec[0].max_gt().map(|x| (x, find_strand(&mrec_vec[0].gt_probs(), x as usize))) };
	// Store record
	let rec = Record::new(brec.rid() as u32, brec.pos() as u32, cx, gt_strand);
	let mut rec_blk = udata.rec_blk.as_mut().unwrap();
	match &mut rec_blk {
		RecordBlock::Single(rb) => rb.push((rec, mrec_vec.pop().unwrap())),
		RecordBlock::Multi(rb) => {
			rb.push((rec, udata.mrec_vec.take().unwrap().into_boxed_slice()));
			udata.mrec_vec = Some(Vec::with_capacity(ns));
		},
	}
	// Send full blocks for output
	if rec_blk.len() == REC_BLOCK_SIZE { send_blk(udata, channel_vec) }
}

pub fn unpack_vcf_slave(chash: Arc<ConfHash>, hdr: Arc<VcfHeader>, channel_vec: Arc<Vec<Sender<(usize, Arc<RecordBlock>)>>>, 
	empty_s: Sender<BrecBlock>, full_r: Receiver<BrecBlock>) {
		
	let ns = hdr.nsamples();
	assert!(ns > 0);
	let mut udata = UnpackData::new(&chash, ns);
	for mut bblk in full_r.iter() {
		let idx = bblk.idx;
		for brec in bblk.buf().iter_mut() {
			unpack_vcf(brec, idx, &hdr, &mut udata, &channel_vec);
		}
		empty_s.send(bblk).expect("Error sending empty buffers");
	}
	send_blk(&mut udata, &channel_vec);
}


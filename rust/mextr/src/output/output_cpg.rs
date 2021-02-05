use std::io::{self, Write};

use libc::c_int;
use r_htslib::{HtsFile, VcfHeader};

use crate::config::*;
use crate::read_vcf::unpack::{RecordBlock, RecordBlockElem, Strand};
use super::{OutputOpts, calc_phred, Record, MethRec, GT_IUPAC, GT_MASK, output_mrec, get_prob_dist};

const HET_C: [bool; 10] = [false, true, false, false, true, true, true, false, false, false];
const HET_G: [bool; 10] = [false, false, true, false, false, true, false, true, true, false];

fn calc_prob(mrec1: &MethRec, mrec2: &MethRec, opts: &OutputOpts) -> (bool, f64) {
	if mrec1.max_gt().is_some() && mrec2.max_gt().is_some() {
		let ct1 = mrec1.counts();
		let p1 = mrec1.gt_probs();
		let ct2 = mrec2.counts();
		let p2 = mrec2.gt_probs();
		let gt1 = mrec1.max_gt.unwrap();
		let gt2 = mrec2.max_gt.unwrap();
		
		if ct1[5] + ct1[7] >= opts.min_inform() || ct2[6] + ct2[4] >= opts.min_inform() {
			if matches!(opts.select, Select::Hom) {
				(gt1 == 4 && gt2 == 7, (p1[4] + p2[7]).exp())
			} else {
				(HET_C[gt1 as usize] && HET_G[gt2 as usize], (p1[1] + p1[4] + p1[5] + p1[6] + p2[2] + p2[5] + p2[7] + p2[8]).exp())
			}
		} else { (false, 0.0) }		
	} else { (false, 0.0) }
}

fn output_combined_mrec<W: Write>(f: &mut W, mrec1: &MethRec, mrec2: &MethRec) -> io::Result<()> {
	if let (Some(gt1), Some(gt2)) = (mrec1.max_gt(), mrec2.max_gt()) {
		let gq = calc_phred(1.0 - (mrec1.gt_probs()[gt1 as usize] + mrec2.gt_probs()[gt2 as usize]).exp());

		// Get weighted geometric average of MQ
		let (n1, n2): (c_int, c_int) = (mrec1.counts.iter().sum(), mrec2.counts.iter().sum());
		let (fn1, fn2) = (n1 as f64, n2 as f64);
		let mq1 = mrec1.mq() as f64;
		let mq2 = mrec2.mq() as f64;
		let	mq = (((mq1 * mq1 * fn1 + mq2 * mq2 * fn2) / (fn1 + fn2)).sqrt() + 0.5) as usize;

		write!(f,"\t{}{}\tGQ={};MQ={}", GT_IUPAC[gt1 as usize] as char, GT_IUPAC[gt2 as usize] as char, gq, mq)?; // TODO - fix mq calc
		if gt1 != 4 || gt2 != 7 { write!(f, ";DQ={}", calc_phred((mrec1.gt_probs()[4] + mrec2.gt_probs[7]).exp()))? }

		// Collect counts
		let (ct0, ct1) = (mrec1.counts[5] + mrec2.counts[6], mrec1.counts[7] + mrec2.counts[4]);
		let (ct2, ct3) = {
			let mut x = 0;
			let mut m = 1;
			let msk1 = GT_MASK[gt1 as usize];
			let msk2 = GT_MASK[gt2 as usize];
			for (c1, c2) in mrec1.counts().iter().zip(mrec2.counts()) {
				if (msk1 & m) != 0 { x += c1 }
				if (msk2 & m) != 0 { x += c2 }
				m <<= 1;
			}
			(x, n1 + n2)
		};
		
		// Get combined meth estimate
		let (m1, m2) = (mrec1.get_meth(Strand::C).unwrap_or(-1.0), mrec2.get_meth(Strand::G).unwrap_or(-1.0));
		let (wt1, wt2) = (if m1 < 0.0 { 0.0 } else { fn1 }, if m2 < 0.0 { 0.0 } else { fn2 });
		// Quick approx - simply get the ratio of psuedocounts
		if wt1 + wt2 > 0.0 { write!(f, "\t{:.3}", (m1 * wt1 + m2 * wt2) / (wt1 + wt2))? } else { write!(f,"\t.")? };
		
		write!(f, "\t{}\t{}\t{}\t{}", ct0, ct1, ct2, ct3)
	} else {
		write!(f, "\t.\t.\t.\t.\t.\t.\t.")	
	}	
}

fn output_separate_single_rec<W: Write>(f: &mut W, hdr: &VcfHeader, rec: &Record, meth_rec: &MethRec, strand: Strand) -> io::Result<()> {
	write!(f, "{}\t{}\t{}\t{}", hdr.ctg_name(rec.rid as usize).unwrap(), rec.pos, rec.pos + 1, rec.cx[2] as char)?;
	output_mrec(f, meth_rec, strand, false)?;
	writeln!(f)
}

fn output_separate_multi_rec<W: Write>(f: &mut W, hdr: &VcfHeader, rec: &Record, mvec: &[MethRec], strand: Strand) -> io::Result<()> {
	write!(f, "{}\t{}\t{}\t{}", hdr.ctg_name(rec.rid as usize).unwrap(), rec.pos, rec.pos + 1, rec.cx[2] as char)?;
	for meth_rec in mvec.iter() { output_mrec(f, meth_rec, strand, false)? }
	writeln!(f)
}

fn output_combined_single_rec<W: Write>(f: &mut W, hdr: &VcfHeader, rec1: &Record, rec2: &Record, meth_rec1: &MethRec, meth_rec2: &MethRec) -> io::Result<()> {
	write!(f, "{}\t{}\t{}\t{}{}", hdr.ctg_name(rec1.rid as usize).unwrap(), rec1.pos, rec2.pos + 1, rec1.cx[2] as char, rec1.cx[3] as char)?;
	output_combined_mrec(f, meth_rec1, meth_rec2)?;
	writeln!(f)
}

fn output_combined_multi_rec<W: Write>(f: &mut W, hdr: &VcfHeader, rec1: &Record, rec2: &Record, mvec1: &[MethRec], mvec2: &[MethRec]) -> io::Result<()> {
	write!(f, "{}\t{}\t{}\t{}{}", hdr.ctg_name(rec1.rid as usize).unwrap(), rec1.pos, rec2.pos + 1, rec1.cx[2] as char, rec1.cx[3] as char)?;
	for (meth_rec1, meth_rec2) in mvec1.iter().zip(mvec2.iter()) { output_combined_mrec(f, meth_rec1, meth_rec2)? }
	writeln!(f)
}

fn output_single_rec<'a, W: Write>(f: &mut W, hdr: &VcfHeader, opts: &OutputOpts, srec: &'a[(Record, MethRec)], mut prev: Option<(&'a Record, &'a MethRec)>) -> io::Result<()> {
	for (rec2, meth_rec2) in srec {
		if let Some((rec1, meth_rec1)) = prev {
			if rec1.gt().is_some() && rec2.gt().is_some() && rec1.rid == rec2.rid && rec1.pos + 1 == rec2.pos {
				let (pass, q) = calc_prob(meth_rec1, meth_rec2, opts);
				if pass { 
					let phred = calc_phred(1.0 - q);
					if opts.min_n() < 1 || phred >= opts.threshold {
						if matches!(opts.mode, Mode::Combined) { output_combined_single_rec(f, hdr, rec1, rec2, meth_rec1, meth_rec2)? }
						else {
							output_separate_single_rec(f, hdr, rec1, meth_rec1, Strand::C)?; 
							output_separate_single_rec(f, hdr, rec2, meth_rec2, Strand::G)?; 
						}
					}
				}
			}
		}
		prev = Some((rec2, meth_rec2));
	}
	Ok(())
}

fn output_multi_rec<'a, W: Write>(f: &mut W, hdr: &VcfHeader, opts: &OutputOpts, srec: &'a [(Record, Box<[MethRec]>)], mut prev: Option<(&'a Record, &'a [MethRec])>) -> io::Result<()> {
	let ns = hdr.nsamples();	
	let mut qvec = Vec::with_capacity(ns);
	for (rec2, mvec2) in srec {
		if let Some((rec1, mvec1)) = prev {
			if rec1.gt().is_some() && rec2.gt().is_some() && rec1.rid == rec2.rid && rec1.pos + 1 == rec2.pos {
				qvec.clear();
				mvec1.iter().zip(mvec2.iter()).map(|(m1, m2)| calc_prob(m1, m2, opts)).for_each(|(_,q)| qvec.push(q));
				get_prob_dist(&mut qvec);
				let phred = calc_phred(qvec[1..opts.min_n].iter().fold(qvec[0], |s, q| s + *q)); 
				if opts.min_n == 0 || phred >= opts.threshold() {
					if matches!(opts.mode, Mode::Combined) { output_combined_multi_rec(f, hdr, rec1, rec2, mvec1, mvec2)? }
					else {
						output_separate_multi_rec(f, hdr, rec1, mvec1, Strand::C)?; 
						output_separate_multi_rec(f, hdr, rec2, mvec2, Strand::G)?; 
					}
				}
			}
		}
		prev = Some((rec2, mvec2));
	}
	Ok(())
}

pub fn output_cpg(outfile: &mut HtsFile, rec_blk: &RecordBlock, prev: Option<RecordBlockElem>, chash: &ConfHash, hdr: &VcfHeader) -> io::Result<()> {
	let opts = OutputOpts::new(chash);
	match rec_blk {
		RecordBlock::Single(svec) => {
			let prev = if let Some(RecordBlockElem::Single((r, m))) = prev { Some((r, m)) } else { None }; 
			output_single_rec(outfile, hdr, &opts, &svec, prev)
		},
		RecordBlock::Multi(mvec) => {
			let prev = if let Some(RecordBlockElem::Multi((r, mv))) = prev { Some((r, mv as &[MethRec])) } else { None }; 
			output_multi_rec(outfile, hdr, &opts, &mvec, prev)
		},
	}
}
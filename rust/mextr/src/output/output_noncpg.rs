use std::io::{self, Write};
use std::str::from_utf8;

use r_htslib::VcfHeader;

use crate::config::*;
use crate::read_vcf::unpack::{RecordBlock, Strand};
use super::{OutputOpts, calc_phred, Record, MethRec};

// Prob. that sample has the required genotype depending on strand:
//  for Select::Hom CC or GG, for Select::Het (AC CC CG CT) or (AG, CG, GG, GT)
fn calc_prob(mrec: &MethRec, strand: Strand, opts: &OutputOpts) -> f64 {
	if mrec.max_gt().is_some() {
		let ct = mrec.counts();
		let p = mrec.gt_probs();
		if let Strand::C = strand {
			if ct[5] >= opts.min_nc() && (ct[5] + ct[7]) >= opts.min_inform() {
				if let Select::Hom = opts.select { p[4].exp() }
				else { p[1].exp() + p[4].exp() + p[5].exp() + p[6].exp() }
			} else { 0.0 }
		} else if ct[6] >= opts.min_nc() && (ct[6] + ct[4]) >= opts.min_inform() {
			if let Select::Hom = opts.select { p[7].exp() }
			else { p[2].exp() + p[5].exp() + p[7].exp() + p[8].exp() }
		} else { 0.0 }			
	} else { 0.0 }
}

const GT_IUPAC: &[u8] = "AMRWCSYGKT".as_bytes();
const GT_MASK: [u8; 10] = [0x11, 0xb3, 0x55, 0x99, 0xa2, 0xf6, 0xaa, 0x54, 0xdc, 0x88];

fn output_mrec<W: Write>(f: &mut W, mrec: &MethRec, strand: Strand) -> io::Result<()> {
	if let Some(gt) = mrec.max_gt() {
		let gq = calc_phred(1.0 - mrec.gt_probs()[gt as usize].exp());
		write!(f,"\t{}\tGQ={};MQ={};CX={}", GT_IUPAC[gt as usize] as char, gq, mrec.mq(), from_utf8(mrec.cx()).unwrap())?;
		let exp_gt = if matches!(strand, Strand::C) { 4 } else { 7 };
		if gt != exp_gt { write!(f, ";DQ={}", calc_phred(mrec.gt_probs()[exp_gt as usize].exp()))? }
		let (ct0, ct1) = if matches!(strand, Strand::C) { (mrec.counts[5], mrec.counts[7]) } else { (mrec.counts[6], mrec.counts[4]) };
		let (ct2, ct3) = {
			let mut x = 0;
			let mut y = 0;
			let mut m = 1;
			let msk = GT_MASK[gt as usize];
			for c in mrec.counts() {
				y += c;
				if (msk & m) != 0 { x += c }
				m <<= 1;
			}
			(x, y)
		};
		let meth = mrec.get_meth(strand).unwrap_or(-1.0);
		write!(f, "\t{}\t{}\t{}\t{}\t{}", meth, ct0, ct1, ct2, ct3)
	} else {
		write!(f, "\t.\t.\t.\t.\t.\t.\t.")	
	}	
}

fn output_single_rec<W: Write>(f: &mut W, hdr: &VcfHeader, opts: &OutputOpts, srec: &[(Record, MethRec)]) -> io::Result<()> {
	for (rec, meth_rec) in srec {
		if let Some(gt) = rec.gt() {
			// Safe to unwrap because of previous line
			let strand = rec.strand().unwrap();
			
			let phred = if opts.min_n() > 0 { calc_phred(1.0 - calc_prob(meth_rec, strand, opts)) } else { 255 };
			if phred >= opts.threshold() {
				write!(f, "{}\t{}\t{}\t{}", hdr.ctg_name(rec.rid as usize).unwrap(), rec.pos, rec.pos + 1, rec.cx[2] as char)?;
				output_mrec(f, meth_rec, strand)?;
				writeln!(f)?;
			} 
		}
	}
	Ok(())
}

fn output_multi_rec<W: Write>(outfile: &mut W, hdr: &VcfHeader, opts: &OutputOpts, mrec: &[(Record, Box<[MethRec]>)]) -> io::Result<()> {
	Ok(())
}

pub fn output_noncpg<W: Write>(outfile: &mut W, rec_blk: &RecordBlock, chash: &ConfHash, hdr: &VcfHeader) -> io::Result<()> {
	
	let opts = OutputOpts::new(chash);
	
	match rec_blk {
		RecordBlock::Single(svec) => output_single_rec(outfile, hdr, &opts, svec)?, 
		RecordBlock::Multi(mvec) => output_multi_rec(outfile, hdr, &opts, mvec)?, 
	}
	
	Ok(())
}
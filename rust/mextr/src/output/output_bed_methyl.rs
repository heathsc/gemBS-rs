use std::io::{self, Write};
use std::str::from_utf8;
use lazy_static::lazy_static;

use r_htslib::{HtsFile, VcfHeader};

use crate::config::*;
use crate::read_vcf::unpack::{RecordBlock, RecordBlockElem};
use super::{OutputOpts, calc_phred, Record, MethRec};

lazy_static! {
	static ref TRANS_BASE: [u8; 256] = {
		let s1 = "ACGTYRSWKMBVDHN.".as_bytes();
		let s2 = "TGCARYSWMKVBHDN.".as_bytes();
		let mut t = [0; 256];
		for (i, j) in s1.iter().zip(s2.iter()) { t[*i as usize] = *j }
		t
	};
}

const BM_TYPE_CPG: usize = 0;
const BM_TYPE_CHG: usize = 1;
const BM_TYPE_CHH: usize = 2;

const RGB_TAB: [&str; 11] = [ "0,255,0", "55,255,0", "105,255,0", "155,255,0", "205,255,0", "255,255,0",
		"255,205,0", "255,155,0", "255,105,0", "255,55,0", "255,0,0" ];
		
fn strand_and_context(rf: &[u8], call: &[u8]) -> Option<(char, [u8; 3], [u8; 3])> {
	let mut ref_cx = [0; 3];
	let mut call_cx = [0; 3];
	match rf[2] {
		b'C' => {
			for (ix, (a, b)) in rf[2..].iter().zip(call[2..].iter()).enumerate() { 
				ref_cx[ix] = *a;
				call_cx[ix] = *b;
			}
			Some(('+', ref_cx, call_cx))
		},
		b'G' => {
			for (ix, (a, b)) in rf[..3].iter().zip(call[..3].iter()).enumerate() { 
				ref_cx[2 - ix] = TRANS_BASE[*a as usize];
				call_cx[2 - ix] = TRANS_BASE[*b as usize];
			}
			Some(('-', ref_cx, call_cx))
		},
		_ => None,
	}
}

fn output_bed_methyl_rec<W: Write>(files: &mut[W], hdr: &VcfHeader, opts: &OutputOpts, srec: &[(Record, MethRec)]) -> io::Result<()> {
	assert_eq!(files.len(), 3);
	let sample_desc = opts.sample_desc().expect("Sample description nont set");
	for (rec, meth_rec) in srec {
		if let Some(gt) = meth_rec.max_gt() {
			let (strand, ref_cx, call_cx) = match strand_and_context(&rec.cx, &meth_rec.cx) {
				Some(v) => v,
				None => continue,
			};
			let (bm_type, rf, call) = {
				if ref_cx[1] == b'G' {
					(BM_TYPE_CPG, &ref_cx[..2], &call_cx[..2])
				} else if ref_cx[2] == b'G' {
					(BM_TYPE_CHG, &ref_cx[..], &call_cx[..])
				} else { (BM_TYPE_CHH, &ref_cx[..], &call_cx[..]) }			
			}; 
			let (a, b) = if strand == '+' { (meth_rec.counts[5], meth_rec.counts[7]) } else { (meth_rec.counts[6], meth_rec.counts[4]) };
			let cov = a + b;
			if cov > 0 {
				let m = (a as f64) / (cov as f64);
				let f = &mut files[bm_type];
				let gq = calc_phred(1.0 - (meth_rec.gt_probs()[gt as usize]).exp()); // Prob of not calling genotype
				writeln!(f, "{}\t{}\t{}\t\"{}\"\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", 
					hdr.ctg_name(rec.rid as usize).unwrap(), rec.pos, rec.pos + 1, sample_desc, cov.min(1000),
					strand, rec.pos, rec.pos + 1, RGB_TAB[(m * 10.0 + 0.5) as usize], cov, (100.0 * m) as usize, 
					from_utf8(rf).unwrap(), from_utf8(call).unwrap(), gq)?;	
			}
		}
	} 
	Ok(())
}

pub fn output_bed_methyl(outfiles: &mut [HtsFile], rec_blk: &RecordBlock, _prev: Option<RecordBlockElem>, chash: &ConfHash, hdr: &VcfHeader) -> io::Result<()> {
	
	let opts = OutputOpts::new(chash);
	match rec_blk {
		RecordBlock::Single(svec) => output_bed_methyl_rec(outfiles, hdr, &opts, svec), 
		RecordBlock::Multi(_) => panic!("Multi sample fils not compatible with bedMethyl"), 
	}
}
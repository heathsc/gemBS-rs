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

fn output_bed_methyl_rec<W: Write>(files: &mut[W], chash: &ConfHash, hdr: &VcfHeader, opts: &OutputOpts, srec: &[(Record, MethRec)], mut prev_ctg: Option<u32>) -> io::Result<()> {
	assert_eq!(files.len(), 3);
	let sample_desc = opts.sample_desc().expect("Sample description not set");
	let bbi_ref = chash.bbi().read().unwrap();
	let bbi = bbi_ref.as_ref().expect("Bbi not set");
	let sender = bbi.sender().expect("Bbi sender not set");

	let bw_strand_specific = matches!(chash.get_mode("bw_mode"), Mode::StrandSpecific);
	let mut bb_builders = Vec::new();

	for f in bbi.bb_files().iter().map(|f| f.build().write().unwrap()) { bb_builders.push(f) }
	let mut bw_builders = Vec::new();
	for f in bbi.bw_files().iter().map(|f| f.build().write().unwrap()) { bw_builders.push(f) }
	if bb_builders.len() != 3 { panic!("Unexpected number of bigBed files")}
	if bw_builders.len() != if bw_strand_specific { 2 } else { 1 } { panic!("Unexpected number of bigWig files")}
	
	for (rec, meth_rec) in srec {
		// For bbi files - handle new ctg
		match prev_ctg {
			Some(old_rid) => {
				if old_rid != rec.rid {
					for build in bb_builders.iter_mut() { 
						build.finish(sender);
						build.clear_counts(); 
					}
					for build in bw_builders.iter_mut() { 
						build.finish(sender); 
						build.clear_counts();
					}
					prev_ctg = Some(rec.rid);
					debug!("Output_bed_methyl - finishing {}, processing {}", chash.vcf_contigs()[old_rid as usize].name(), chash.vcf_contigs()[rec.rid as usize].name());
				}
			},
			None => {
				prev_ctg = Some(rec.rid);
				debug!("Output_bed_methyl - processing {}", chash.vcf_contigs()[rec.rid as usize].name());
			},
		}
			
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
				let sbuf = format!("\"{}\"\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", 
					sample_desc, cov.min(1000), strand, 
					rec.pos, rec.pos + 1, RGB_TAB[(m * 10.0 + 0.5) as usize], cov, (100.0 * m) as usize, 
					from_utf8(rf).unwrap(), from_utf8(call).unwrap(), gq);	

				writeln!(f, "{}\t{}\t{}\t{}",	hdr.ctg_name(rec.rid as usize).unwrap(), rec.pos, rec.pos + 1, &sbuf)?;	

				// Handle bbi files
				//
				
				let out_ix = chash.vcf_contigs()[rec.rid as usize].out_ix().expect("Missing out index for contig");
				// Handle bb record 
				let bb_build = &mut bb_builders[bm_type];
				bb_build.add_bb_rec(out_ix as u32, rec.pos, &sbuf, sender);
				bb_build.add_zoom_obs(out_ix as u32, rec.pos, m as f32, sender);
				// Handle bw record
				let bw_build = &mut bw_builders[ if bw_strand_specific && strand == '-' { 1 } else { 0 } ];
				bw_build.add_bw_rec(out_ix as u32, rec.pos, m as f32, sender);
				bw_build.add_zoom_obs(out_ix as u32, rec.pos, m as f32, sender);
			}
		}
	} 
	Ok(())
}

pub fn output_bed_methyl(outfiles: &mut [HtsFile], rec_blk: &RecordBlock, prev: Option<RecordBlockElem>, chash: &ConfHash, hdr: &VcfHeader) -> io::Result<()> {
	
	let opts = OutputOpts::new(chash);
	let prev_ctg = prev.map(|x| x.record().rid());
	match rec_blk {
		RecordBlock::Single(svec) => output_bed_methyl_rec(outfiles, chash, hdr, &opts, svec, prev_ctg), 
		RecordBlock::Multi(_) => panic!("Multi sample files not compatible with bedMethyl"), 
	}
}
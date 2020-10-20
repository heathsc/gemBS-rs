use std::sync::{Arc, mpsc};
use std::{cmp, io, thread};

use crate::config::{BsCallConfig, BsCallFiles};
use super::pileup::Pileup;
use crate::stats::StatJob;
use super::vcf::{write_vcf_entry, WriteVcfJob};
use crate::htslib::hts_err;

mod model;
mod fisher;

use model::Model;
use fisher::FisherTest;

pub enum CallEntry {
	Call(GenotypeCall),
	Skip(u8),
}

pub struct GenotypeCall {
	pub counts: [usize; 8],
	pub gt_ll: [f64; 10],
	pub fisher_strand: f64,
	pub qual: [usize; 8],
	pub mq: u8,
	pub aq: u8,
	pub max_gt: u8,
	pub ref_base: u8,
}

pub struct CallBlock {
	pub start: usize,
	pub sam_tid: usize,
	pub prec_ref_bases: [u8; 2], // the 2 reference bases before the block begins (or N if not present) 
}

const BLOCK_SIZE: usize = 4096;

fn call_from_pileup(pileup: Pileup, model: &Model, fisher: &FisherTest, write_tx: &mpsc::Sender<WriteVcfJob>) -> io::Result<()> {
	
	let call_block = CallBlock{start: pileup.start, sam_tid: pileup.sam_tid, prec_ref_bases: pileup.get_prec_2_bases()};
	send_write_job(WriteVcfJob::CallBlock(call_block), write_tx)?;
	// Send call_block to output thread
	let mut call_vec = Vec::with_capacity(BLOCK_SIZE);
	for (pp, ref_base) in pileup.data.iter().zip(pileup.get_ref_iter()) {
		let mut counts: [usize; 8] = [0; 8];
		let total = pp.counts.iter().map(|x| *x as usize).enumerate().fold(0, |s, (i, x)| { counts[i & 7] += x; s + x} );
		let call = if total > 0 {
			let total_flt = total as f32;
			let mut qual: [usize; 8] = [0; 8];
			let total_qual = counts.iter().enumerate().filter(|(_, n)| *n > &0).fold(0.0, |s, (i, n)| {
				qual[i] = cmp::min((pp.quality[i] / (*n as f32)).round() as usize, 63);
				s + pp.quality[i]
			});
			let aq = cmp::min((total_qual / (total_flt as f32)).round() as usize, 255) as u8;
			let mq = cmp::min((pp.mapq2 / (total_flt as f32)).sqrt().round() as usize, 255) as u8;
			let (mx, gt_ll) = model.calc_gt_prob(&counts, &qual, *ref_base);
			let fisher_strand = fisher.calc_fs_stat(mx, &pp.counts, model.ln_10());
			CallEntry::Call(GenotypeCall{counts, gt_ll, fisher_strand, qual, mq, aq, max_gt: mx as u8, ref_base: *ref_base})
		} else { CallEntry::Skip(*
		ref_base) };
		call_vec.push(call);
		if call_vec.len() == BLOCK_SIZE {
			send_write_job(WriteVcfJob::GenotypeCall(call_vec), write_tx)?;	
			call_vec = Vec::with_capacity(BLOCK_SIZE);
		}	
	}
	if !call_vec.is_empty() { send_write_job(WriteVcfJob::GenotypeCall(call_vec), write_tx)?; }
	Ok(())	
}

fn send_write_job(job: WriteVcfJob, write_tx: &mpsc::Sender<WriteVcfJob>) -> io::Result<()> {
	match write_tx.send(job) { 
		Err(e) => {
			warn!("Error trying to send new task to write_vcf thread");
			Err(hts_err(format!("Error sending region to write_vcf thread: {}", e)))
		},
		Ok(_) => Ok(()),
	} 	
}

pub fn call_genotypes(bs_cfg: Arc<BsCallConfig>, rx: mpsc::Receiver<Option<Pileup>>, bs_files: BsCallFiles, stat_tx: mpsc::Sender<StatJob>) {
	info!("call_genotypes_thread starting up");
	let ref_bias = bs_cfg.conf_hash.get_float("reference_bias");
	let conversion = (bs_cfg.conf_hash.get_float("under_conversion"), bs_cfg.conf_hash.get_float("over_conversion"));
	let (write_tx, write_rx) = mpsc::channel();
	let write_handle = thread::spawn(move || { write_vcf_entry(Arc::clone(&bs_cfg), write_rx, bs_files, stat_tx) });
	let model = Model::new(63, conversion, ref_bias);
	let fisher = FisherTest::new();
	loop {
		match rx.recv() {
			Ok(None) => break,
			Ok(Some(pileup)) => {
				debug!("Received new pileup: {}:{}-{}", pileup.sam_tid, pileup.start, pileup.start + pileup.data.len() - 1);
				if let Err(e) = call_from_pileup(pileup, &model, &fisher, &write_tx) {
					error!("call_from_pileup failed with error: {}", e);
					break;
				}
			},
			Err(e) => {
				warn!("call_genotypes thread recieved error: {}", e);
				break
			}
		}
	}
	if write_tx.send(WriteVcfJob::Quit).is_err() { warn!("Error trying to send QUIT signal to write_vcf thread") }
	if write_handle.join().is_err() { warn!("Error waiting for call_genotype thread to finish") }
	info!("call_genotypes thread shutting down");	
}
use std::sync::{Arc, mpsc};

use crate::config::BsCallConfig;
use super::records::ReadEnd;
use crate::htslib;

pub struct PileupRegion {
	start: usize,
	end: usize,
	cname: String,
	reads: Vec<Option<ReadEnd>>,
}

impl PileupRegion {
	pub fn new<S: AsRef<str>>(cname: S, start: usize, end: usize, reads: Vec<Option<ReadEnd>>) -> Self {
		Self{cname: cname.as_ref().to_owned(), start, end, reads}
	}	
}

fn handle_pileup(bs_cfg: Arc<BsCallConfig>, preg: PileupRegion, seq: &mut Option<htslib::Sequence>) -> Result<(), ()> {
	
	Ok(())
}

pub fn make_pileup(bs_cfg: Arc<BsCallConfig>, rx: mpsc::Receiver<Option<PileupRegion>>) {
	info!("pileup_thread starting up()");
	let mut seq: Option<htslib::Sequence> = None;
	loop {
		match rx.recv() {
			Ok(None) => break,
			Ok(Some(preg)) => {
				debug!("Received new pileup region: {}:{}-{}", preg.cname, preg.start, preg.end);
				if handle_pileup(Arc::clone(&bs_cfg), preg, &mut seq).is_err() { break; }
			},
			Err(e) => {
				warn!("make_pileup_thread recieved error: {}", e);
				break
			}
		}
	}
	info!("pileup_thread shutting down()");
}	

use std::{io, thread};
use std::sync::{Arc, mpsc};

use crate::config::{BsCallConfig, BsCallFiles};
use crate::stats;

pub mod vcf;
pub mod sam;
pub mod records;
pub mod read_data;
pub mod pileup;
pub mod call_genotypes;

pub use vcf::*;
pub use sam::*;

pub const GT_HET: [bool; 10] = [false, true, true, true, false, true, true, false, true, false];

pub fn process(bs_cfg: Arc<BsCallConfig>, bs_files: BsCallFiles) -> io::Result<()> {
	// Set up stats logging thread
	let (stats_tx, stats_rx) = mpsc::channel();
	let sname = bs_cfg.get_conf_str("report_file").map(|s| s.to_owned());
	let source = bs_cfg.get_conf_str("bs_call_source").expect("No bs_call_source variable").to_owned();
	let stat_handle = thread::spawn(move || { stats::stat_thread(sname, source, stats_rx) });
	read_data::read_data(Arc::clone(&bs_cfg), mpsc::Sender::clone(&stats_tx), bs_files)?;
	
	if stats_tx.send(stats::StatJob::Quit).is_err() { warn!("Error trying to sent QUIT signal to stats thread") }
	else if stat_handle.join().is_err() { warn!("Error waiting for stats thread to finish") }
	Ok(())
}

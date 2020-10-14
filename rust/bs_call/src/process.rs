use std::{io, thread};
use std::sync::mpsc;

use crate::config::BsCallConfig;
use crate::stats;

pub mod vcf;
pub mod sam;
pub mod records;
pub mod read_data;

pub use vcf::*;
pub use sam::*;

pub fn process(bs_cfg: &mut BsCallConfig) -> io::Result<()> {
	// Set up stats logging thread
	let mut stats = bs_cfg.stats.take();
	let (stats_tx, stats_rx) = mpsc::channel();
	let stat_handle = thread::spawn(move || { stats::stat_thread(&mut stats, stats_rx) });
	
	read_data::read_data(bs_cfg, mpsc::Sender::clone(&stats_tx))?;
	
	if stats_tx.send(stats::StatJob::Quit).is_err() { warn!("Error trying to sent QUIT signal to stats thread") }
	else if stat_handle.join().is_err() { warn!("Error waiting for stats thread to finish") }
	Ok(())
}

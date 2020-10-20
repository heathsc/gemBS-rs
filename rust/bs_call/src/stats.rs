use chrono::prelude::*;
use std::sync::mpsc;

pub mod stats_json;
pub mod meth_profile;
pub use stats_json::{FSReadLevelType, FSBaseLevelType, FSType, FSCounts};
pub use meth_profile::{MethProfile};

pub struct Stats {
	stats: stats_json::CallJson,
	output_file: String,
}

impl Stats {
	pub fn new<S: AsRef<str>, U: AsRef<str>>(ofile: U, source: S) -> Self {
		let date = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
		Self {
			stats: stats_json::CallJson::new(source, date),
			output_file: ofile.as_ref().to_owned(),
		}
	} 
}

impl Drop for Stats {
	fn drop(&mut self) {
		match utils::compress::open_bufwriter(&self.output_file) {
			Ok(wrt) => if let Err(e) = self.stats.to_writer(wrt) {	eprintln!("Error when writing out JSON stats file {}: {}", self.output_file, e); },
			Err(e) =>  eprintln!("Couldn't open JSON stats file{} for writing: {}", self.output_file, e),
		}
	}
}

pub enum StatJob {
	AddFSReadLevelCounts(FSReadLevelType, FSCounts),
	AddFSBaseLevelCounts(FSBaseLevelType, usize),
	SetNonCpgReadProfile(Vec<[usize; 4]>),
	Quit,
}

fn accumulate_stats(stats: &mut stats_json::CallJson, rx: mpsc::Receiver<StatJob>) {
	info!("stat_thread starting up");
	loop {
		match rx.recv() {
			Ok(StatJob::Quit) => break,
			Ok(StatJob::AddFSReadLevelCounts(fs_type, c)) => stats.filter_stats().add_read_level_fs_counts(fs_type, c),
			Ok(StatJob::AddFSBaseLevelCounts(fs_type, c)) => stats.filter_stats().add_base_level_count(fs_type, c),
			Ok(StatJob::SetNonCpgReadProfile(v)) => stats.total_stats().methylation().non_cpg_read_profile = Some(v),
			Err(e) => {
				warn!("stat_thread recieved error: {}", e);
				break
			}
		}
	}
	info!("stat_thread shutting down");
}

// No output stats file supplied so we just throw away all messages apart from Quit
fn dummy_job(rx: mpsc::Receiver<StatJob>) {
	debug!("dummy stat_thread starting up");
	loop {
		match rx.recv() {
			Ok(StatJob::Quit) => break,
			Err(e) => {
				warn!("stat_thread recieved error: {}", e);
				break
			}
			_ => (),
		}
	}
	debug!("dummy stat_thread shutting down");
}

pub fn stat_thread(stats_name: Option<String>, source: String, rx: mpsc::Receiver<StatJob>) {
	// Initialize Stats
	if let Some(name) = stats_name {
		let mut s = Stats::new(name, source);
		accumulate_stats(&mut s.stats, rx);
	} else { dummy_job(rx) }
}

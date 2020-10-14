use chrono::prelude::*;
use std::sync::mpsc;

pub mod stats_json;
pub use stats_json::{FSReadLevelType, FSBaseLevelType, FSType, FSCounts};

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
	Quit,
}

fn accumulate_stats(stats: &mut stats_json::CallJson, rx: mpsc::Receiver<StatJob>) {
	info!("stat_thread starting up()");
	loop {
		match rx.recv() {
			Ok(StatJob::Quit) => break,
			Ok(StatJob::AddFSReadLevelCounts(fs_type, c)) => stats.filter_stats().add_read_level_fs_counts(fs_type, c),
			Ok(StatJob::AddFSBaseLevelCounts(fs_type, c)) => stats.filter_stats().add_base_level_count(fs_type, c),
			Err(e) => {
				warn!("stat_thread recieved error: {}", e);
				break
			}
		}
	}
	info!("stat_thread shutting down()");
}

// No output stats file supplied so we just throw away all messages apart from Quit
fn dummy_job(rx: mpsc::Receiver<StatJob>) {
	info!("dummy stat_thread starting up()");
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
	info!("dummy stat_thread shutting down()");
}

pub fn stat_thread(stats: &mut Option<Stats>, rx: mpsc::Receiver<StatJob>) {
	if let Some(s) = stats.as_mut() { accumulate_stats(&mut s.stats, rx); }
	else { dummy_job(rx) }
}
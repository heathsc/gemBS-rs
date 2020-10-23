use chrono::prelude::*;
use std::sync::mpsc;

pub mod stats_json;
pub mod meth_profile;
pub mod vcf_stats;

pub use stats_json::*;
pub use meth_profile::MethProfile;
pub use vcf_stats::{collect_vcf_stats, VcfStats, MUT_NAMES, SITE_TYPE_ALL, SITE_TYPE_VARIANT, SITE_TYPE_CPG_REF, SITE_TYPE_CPG_NON_REF};

pub struct Stats {
	stats: stats_json::CallJson,
	output_file: String,
}

impl Stats {
	pub fn new<S: AsRef<str>, U: AsRef<str>>(ofile: U, source: S, vcf_stats: Option<VcfStats>, filter_stats: FSType) -> Self {
		let date = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
		Self {
			stats: stats_json::CallJson::from_stats(source, date, vcf_stats, filter_stats),
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
	AddVcfStats(VcfStats),
	Quit,
}

fn accumulate_stats(name: String, source: String, rx: mpsc::Receiver<StatJob>) {
	info!("stat_thread starting up");
	let mut filter_stats = FSType::new();
	let mut non_cpg_read_profile: Option<Vec<[usize; 4]>> = None;
	let mut vcf_stats: Option<VcfStats> = None;
	loop {
		match rx.recv() {
			Ok(StatJob::Quit) => {
				debug!("stat_thread recieved quit signal - generating report");
				let mut stats = Stats::new(name, source, vcf_stats, filter_stats);
				stats.stats.total_stats().methylation().non_cpg_read_profile = non_cpg_read_profile;
				break;
			},
			Ok(StatJob::AddFSReadLevelCounts(fs_type, c)) => filter_stats.add_read_level_fs_counts(fs_type, c),
			Ok(StatJob::AddFSBaseLevelCounts(fs_type, c)) => filter_stats.add_base_level_count(fs_type, c),
			Ok(StatJob::SetNonCpgReadProfile(v)) => non_cpg_read_profile = Some(v),
			Ok(StatJob::AddVcfStats(vs)) => vcf_stats = Some(vs),
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
	if let Some(name) = stats_name { accumulate_stats(name, source, rx) } else { dummy_job(rx) }
}

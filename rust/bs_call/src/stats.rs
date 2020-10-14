use chrono::prelude::*;

pub mod stats_json;

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
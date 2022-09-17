#[macro_use]
extern crate log;

use std::sync::Arc;

mod cli;
pub mod defs;
pub mod config;
pub mod reference;
pub mod htslib;
pub mod process;
pub mod stats;
pub mod dbsnp;
pub mod rusage;

use rusage::*;

fn main() -> Result<(), &'static str> {
	let (bs_cfg, bs_files) = match cli::process_cli() {
		Err(e) => {
			error!("bs_call initialization failed with error: {}", e);
			return Err("Failed");
		},
		Ok(x) => x,
	};

	if let Err(e) = process::process(Arc::new(bs_cfg), bs_files) {
		error!("bs_call ended with error: {}", e);
		Err("Failed")
	} else { 
		if let Ok(ru_thread) = Rusage::get(RusageWho::RusageThread) {
			info!("CPU usage main thread: user {}, sys {}", ru_thread.utime(), ru_thread.stime());
		}
		let ru_self = Rusage::get(RusageWho::RusageSelf).unwrap();
		info!("CPU usage total: user {}, sys {}", ru_self.utime(), ru_self.stime());
		Ok(()) 
	}
}

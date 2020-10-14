#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod cli;
pub mod defs;
pub mod config;
pub mod reference;
pub mod htslib;
pub mod process;
pub mod stats;

fn main() -> Result<(), &'static str> {
	let mut bs_cfg = match cli::process_cli() {
		Err(e) => {
			error!("bs_call initialization failed with error: {}", e);
			return Err("Failed");
		},
		Ok(x) => x,
	};
	if let Err(e) = process::process(&mut bs_cfg) {
		error!("bs_call ended with error: {}", e);
		Err("Failed")
	} else { Ok(()) }
}

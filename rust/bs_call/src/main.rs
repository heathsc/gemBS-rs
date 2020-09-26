#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod cli;
pub mod defs;
pub mod config;
pub mod sam;
pub mod reference;
pub mod htslib;

fn main() -> Result<(), &'static str> {
	let bs_cfg = match cli::process_cli() {
		Ok(c) => c,
		Err(e) => {
			error!("bs_call exited with error: {}", e);
			return Err("Stopped");
		}
	};
	
	Ok(())
}

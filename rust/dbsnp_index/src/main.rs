#[macro_use]
extern crate log;

mod cli;
mod process;
pub mod snp;
pub mod contig;
pub mod config;
pub mod read;
pub mod write;
pub mod compress;

fn main()  -> Result<(), String> {
	let (conf, files) = cli::process_cli().map_err(|e| format!("dbsnp_index initialization failed with error: {}", e))?;
	match process::process(conf, files) {
		Ok(_) => Ok(()),
		Err(e) => {
			error!("dbsnp failed with error: {}", e);
			Err("Failed".to_string())
		}
	}
}

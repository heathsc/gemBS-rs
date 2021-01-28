#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod cli;
pub mod config;
pub mod dbsnp;
pub mod process;
pub mod md5;
pub mod tabix;

fn main()  -> Result<(), String> {
	let conf = cli::process_cli().map_err(|e| format!("dbsnp_index initialization failed with error: {}", e))?;
	match process::process(conf) {
		Ok(_) => Ok(()),
		Err(e) => {
			error!("dbsnp failed with error: {}", e);
			Err("Failed".to_string())
		}
	}
}

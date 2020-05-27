#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod cli;
mod config;
mod commands;
mod common;
use crate::common::defs::Section;

fn main() -> Result<(), String> {
	let mut gem_bs = config::GemBS::new();
	match cli::process_cli(&mut gem_bs) {
		Ok(_) => {
			let rv = gem_bs.get_config(Section::Mapping, "bam_dir");
			println!("bam_dir: {:?}", rv);
/*			let href = gem_bs.get_sample_data_ref();
			for(ds, rf) in href.iter() {
				println!("{}: {:?}", ds, rf);
			} */
			
			// gem_bs.to_json();
			Ok(())
		},
		Err(e) => Err(e),
	}
}

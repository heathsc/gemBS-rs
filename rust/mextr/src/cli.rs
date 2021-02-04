use std::io;

use utils::log_level::init_log;
use clap::App;
mod options;
pub mod cli_utils;

use r_htslib::BcfSrs;
use crate::config::ConfHash;


pub fn process_cli() -> io::Result<(ConfHash, BcfSrs)> {
	let yaml = load_yaml!("cli/cli.yml");
    let app = App::from_yaml(yaml).version(crate_version!());
	
	// Setup logging
	let m = app.get_matches();	
	let _ = init_log(&m);
	// Process arguments
	options::handle_options(&m)
}

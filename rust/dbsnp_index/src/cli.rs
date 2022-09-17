use std::io;

mod cli_utils;
use utils::log_level::init_log;
mod options;
mod cli_model;

use crate::config::{Config, DbInput};

pub fn process_cli() -> io::Result<(Config, Box<[DbInput]>)> {
    let app = cli_model::cli_model();
	
	// Setup logging
	let m = app.get_matches();	
	let _ = init_log(&m);
	// Process arguments
	options::handle_options(&m)
}

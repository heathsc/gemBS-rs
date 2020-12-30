use std::io;


mod cli_utils;
use utils::log_level::init_log;
use clap::App;
mod options;
use crate::config::Config;

pub fn process_cli() -> io::Result<(Config, Box<[String]>)> {
	let yaml = load_yaml!("cli/cli.yml");
    let app = App::from_yaml(yaml).version(crate_version!());
	
	// Setup logging
	let m = app.get_matches();	
	let _ = init_log(&m);
	// Process arguments
	options::handle_options(&m)
}

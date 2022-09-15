use std::io;

use utils::log_level::init_log;
mod cli_model;
mod options;

use crate::config::Config;

pub fn process_cli() -> io::Result<Config> {
    let app = cli_model::cli_model();

    // Setup logging
    let m = app.get_matches();
    let _ = init_log(&m);
    // Process arguments
    options::handle_options(&m)
}

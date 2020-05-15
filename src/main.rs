#[macro_use]
extern crate log;
extern crate stderrlog;
#[macro_use]
extern crate clap;

mod cli;

fn main() {
	cli::process_cli();
    trace!("trace message");
    debug!("debug message");
    info!("info message");
    warn!("warn message");
    error!("error message");
}
#[macro_use]
extern crate log;
extern crate stderrlog;
#[macro_use]
extern crate clap;

mod cli;
mod commands;

fn main() -> Result<(), &'static str> {
	cli::process_cli()
}

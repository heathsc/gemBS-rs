#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod cli;
mod commands;
mod common;

fn main() -> Result<(), String> {
	cli::process_cli()
}

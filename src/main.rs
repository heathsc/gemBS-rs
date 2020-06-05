#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod cli;
mod config;
mod commands;
mod common;

fn main() -> Result<(), String> {
	let mut gem_bs = config::GemBS::new();
	cli::process_cli::process_cli(&mut gem_bs)
}

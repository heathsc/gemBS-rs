#[macro_use]
extern crate log;

mod cli;
#[cfg(feature = "slurm")]
mod cluster_mgmt;
mod commands;
mod common;
mod config;
mod scheduler;

fn main() -> Result<(), String> {
    let mut gem_bs = config::GemBS::new();
    cli::process_cli::process_cli(&mut gem_bs)
}

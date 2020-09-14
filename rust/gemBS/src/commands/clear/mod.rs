use std::io;
use std::io::prelude::*;

use clap::ArgMatches;
use crate::cli::utils::handle_options;
use crate::config::GemBS;
use crate::common::defs::Section;
use crate::common::utils;
use crate::common::assets::GetAsset;

pub fn clear_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
	gem_bs.setup_fs(false)?;
	// Get config file from disk
	gem_bs.read_config()?;
	
	let options = handle_options(m, gem_bs, Section::Report);
	if !options.contains_key("_confirm") {
		println!("Warning: This command must not be run if other gemBS commands are executing on the same directory");
		println!("Please enter 'y' to continue");
		let stdin = io::stdin();
		let mut s = String::new();
		stdin.lock().read_line(
			&mut s).map_err(|e| format!("{}", e))?;
		if !s.to_lowercase().starts_with('y') { return Ok(()); }
	}
	let task_path = gem_bs.get_task_file_path();
	let t = if options.contains_key("_force") { utils::FileLock::new_force(&task_path) }
	else { utils::FileLock::new(&task_path) };
	let flock = match t {
		Ok(f) => f,
		Err(e) => {
			if e.starts_with("File locked") {
				return Err(format!("Could not obtain lock: {}.\nIf you are sure that no other process is running on this gemBS directory then re-run with the --force option", e));
			} else {
				return Err(e);
			}
		},
	};
	gem_bs.setup_assets_and_tasks(&flock)?;
	let running = crate::config::get_running_tasks(&flock)?;
	for rtask in running.iter() {
		debug!("Found incomplete task: {}", rtask.id());
		match gem_bs.get_tasks().find_task(rtask.id()) {
			Some(task_ix) => {
				let task = &gem_bs.get_tasks()[task_ix];
				for ix in task.outputs() {
					match gem_bs.get_assets().get_asset(*ix) {
						Some(asset) => {
							debug!(" - Output asset for task: {}", asset.path().display());
							if asset.path().exists() {
								info!("Removing incomplete output file {}", asset.path().display());
								let _ = std::fs::remove_file(asset.path());
							}
						},
						None => warn!("Unknown asset as output from running task"),
					}
				}
			},
			None => warn!("Unknown task {} in running list", rtask.id())
		}
	}
	if flock.path().exists() {
		info!("Removing list of incomplete tasks");
		let _ = std::fs::remove_file(flock.path());
	}
	Ok(())
}
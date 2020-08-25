use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::BufWriter;
use crate::config::GemBS;
use crate::common::tasks::{Task, JsonTask};
use crate::common::defs::{Command, DataValue};
use crate::common::assets::GetAsset;
use std::path::Path;

// Prepare job graph and submit to slurm
pub fn handle_slurm(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>, task_list: &[usize]) -> Result<(), String> {
	
	Ok(())
}

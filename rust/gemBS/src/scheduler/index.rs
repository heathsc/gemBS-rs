use std::collections::HashMap;

use crate::config::GemBS;
use crate::common::assets::GetAsset;
use crate::common::defs::{DataValue, Section};
use super::QPipe;

fn make_gem_index(gem_bs: &GemBS, job: usize, bisulfite: bool) -> QPipe
{
	let gembs_ref = gem_bs.get_asset("gembs_reference").expect("Couldn't find gemBS reference asset");
	let index_name = if bisulfite { "index" } else { "nonbs_index" };
	let index = gem_bs.get_asset(index_name).expect("Couldn't find gemBS index asset").path();
	let index_base = index.to_string_lossy().clone();
	let index_base = index_base.trim_end_matches(".gem");
	let gem_indexer = gem_bs.get_exec_path("gem-indexer");
	let mut args = format!("-i\x1e{}\x1e-o\x1e{}\x1e", gembs_ref.path().to_string_lossy(), index_base);
	if bisulfite {args.push_str("--bisulfite-index\x1e")}
	if let Some(x) = gem_bs.get_config_int(Section::Index, "sampling_rate") { args.push_str(format!("--text-sampling-rate\x1e{}\x1e", x).as_str())}
	if let Some(x) = gem_bs.get_config_int(Section::Index, "threads") { args.push_str(format!("--threads\x1e{}\x1e", x).as_str())}
	if let Some(x) = index.parent() {  args.push_str(format!("--tmp-folder\x1e{}", x.to_string_lossy()).as_str())}
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	if let Some(x) = gem_bs.get_tasks()[job].log() { pipeline.log = Some(gem_bs.get_asset(x).expect("Couldn't get log file").path().to_owned()) }
	if gem_bs.get_config_bool(Section::Index, "keep_logs") { pipeline.set_remove_log(false) }
	for out in gem_bs.get_tasks()[job].outputs() { pipeline.add_outputs(gem_bs.get_asset(*out).expect("Couldn't get md5sum output asset").path()); }
	pipeline.add_stage(&gem_indexer, &args);	
	pipeline	
}

fn make_dbsnp_index(gem_bs: &GemBS, _options: &HashMap<&'static str, DataValue>, _job: usize) -> QPipe
{
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	if gem_bs.get_config_bool(Section::Index, "keep_logs") { pipeline.set_remove_log(false) }
	pipeline	
}

pub fn make_index_pipeline(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>, job: usize) -> QPipe
{
	// First check what type of index we are meant to be building
	match gem_bs.get_tasks()[job].id() {
		"index" => make_gem_index(gem_bs, job, true),
		"nonbs_index" => make_gem_index(gem_bs, job, false),
		"dbsnp_index" => make_dbsnp_index(gem_bs, options, job),
		_ => panic!("Unknown index task")
	}	
}
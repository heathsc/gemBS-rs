use std::path::Path;
use crate::config::GemBS;
use crate::common::assets::GetAsset;
use super::QPipe;

pub fn make_md5sum_pipeline(gem_bs: &GemBS, job: usize) -> QPipe
{
	let task = &gem_bs.get_tasks()[job];
	let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
	let md5sum = Path::new("md5sum");
	if task.inputs().len() != 1 { panic!("Expecting one input file for md5sum command"); }
	let input = gem_bs.get_asset(*(task.inputs().next().unwrap())).expect("Couldn't get input asset for md5sum").path().to_string_lossy();
	if task.outputs().len() != 1 { panic!("Expecting one output file for md5sum command"); }
	let output = gem_bs.get_asset(*(task.outputs().next().unwrap())).expect("Couldn't get output asset for md5sum").path();
	let opt_out = Some(output.to_owned());
	pipeline.set_output(opt_out);
	pipeline.add_stage(md5sum, &input);
	pipeline		
}
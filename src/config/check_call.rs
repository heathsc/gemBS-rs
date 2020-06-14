// Check requirements and presence of source and derived files for calling
// Make asset list for BAMs, BCFs etc. associated with calling
use std::path::{Path, PathBuf};
use crate::common::defs::{Section, DataValue, Command};
use crate::common::assets::{AssetType, GetAsset};
use super::GemBS;
use super::contig;

pub fn check_call(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Mapping, name ) { x } else { "." } };
	let bcf_dir = get_dir("bcf_dir").to_owned();
	let make_cram = gem_bs.get_config_bool(Section::Mapping, "make_cram");
	let ext = if make_cram { "cram" } else {"bam" };
	let samples = gem_bs.get_samples();
	let pools = contig::get_contig_pools(gem_bs);
	let mut common_inputs = Vec::new();
	for f in &["gembs_reference", "gembs_reference_fai", "gembs_reference_gzi"] {
		if let Some(x) = gem_bs.get_asset(*f) { common_inputs.push(x.idx()) } else { panic!("{} not found", f) };
	}
	if let Some(x) = gem_bs.get_asset("dbsnp_index") { common_inputs.push(x.idx()) } 
	let handle_file = |gb: &mut GemBS, nm: String, p: &Path| {
		let tpath = Path::new(nm.as_str());
		let path: PathBuf = [p, tpath].iter().collect();
		gb.insert_asset(nm.as_str(), &path, AssetType::Derived)
	}; 
	for (bcode, name) in samples.iter() {
		let bam = if let Some(x) = gem_bs.get_asset(format!("{}.{}", bcode, ext).as_str()) { x.idx() } 
		else { panic!("alignment file {}.{} not found", bcode, ext); };
		let replace_meta_var = |s: &str| {
			if let Some(sm) = name { s.replace("@BARCODE", bcode).replace("@SAMPLE", sm) } else { s.replace("@BARCODE", bcode) }
		};
		let tmp_bcf_dir = replace_meta_var(&bcf_dir);
		let bcf_path = Path::new(&tmp_bcf_dir);	
		if pools.len() > 1 {
			let mut out_bcfs = Vec::new();
			for pool in pools.iter() {
				let mut in_vec = common_inputs.clone();
				in_vec.push(bam);
				let out = handle_file(gem_bs, format!("{}_{}.bcf", bcode, pool), bcf_path);
				let out1 = handle_file(gem_bs, format!("{}_{}.json", bcode, pool), bcf_path);
				out_bcfs.push(out);
				let call_task = gem_bs.add_task(format!("bcf_call_{}_{}", bcode, pool).as_str(), 
					format!("Call BCFs for pool {}, barcode {}", pool, bcode).as_str(),
				 	Command::Call, format!("--barcode {} --pool {}", bcode, pool).as_str(), &in_vec, &[out, out1]);
				[out, out1].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(call_task, &in_vec));
			}
			let out = handle_file(gem_bs, format!("{}.bcf", bcode), bcf_path);
			let merge_task = gem_bs.add_task(format!("bcf_merge_{}", bcode).as_str(), 
					format!("Merge BCFs for barcode {}", bcode).as_str(),
				 	Command::MergeBcfs, format!("--barcode {}", bcode).as_str(), &out_bcfs, &[out]);
			gem_bs.get_asset_mut(out).unwrap().set_creator(merge_task, &out_bcfs);
		} else {
			let mut in_vec = common_inputs.clone();
			in_vec.push(bam);
			let out = handle_file(gem_bs, format!("{}.bcf", bcode), bcf_path);
			let out1 = handle_file(gem_bs, format!("{}.json", bcode), bcf_path);
			let call_task = gem_bs.add_task(format!("bcf_call_{}", bcode).as_str(), 
					format!("Call BCFs for barcode {}", bcode).as_str(),
				 	Command::Call, format!("--barcode {}", bcode).as_str(), &in_vec, &[out, out1]);
			[out, out1].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(call_task, &in_vec));
		}
	}
	Ok(())
}
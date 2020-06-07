// Check requirements and presence of source and derived files for calling
// Make asset list for BAMs, BCFs etc. associated with calling
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::rc::Rc;
use crate::common::defs::{Metadata, Section, DataValue, Command, ContigInfo};
use crate::common::assets::{AssetType, GetAsset};
use super::GemBS;

fn get_samples(gem_bs: &GemBS) -> Vec<(String, Option<String>)> {
	let mut bc_set = HashMap::new();
	let href = gem_bs.get_sample_data_ref();	
	for (dataset, href1) in href.iter() {
		let name = href1.get(&Metadata::SampleName).and_then(|x| {
			if let DataValue::String(s) = x {Some(s) } else { None }
		});
		if let Some(DataValue::String(bcode)) = href1.get(&Metadata::SampleBarcode) {
			bc_set.insert(bcode, name);
		} else { panic!("No barcode associated with dataset {}", dataset); }
	}	
	let mut sample = Vec::new();
	for (bc, name) in bc_set.iter() {
		let n = if let Some(x) = name {Some((*x).to_owned())} else {None};
		sample.push(((*bc).clone(), n));
	}
	sample
}

fn get_contig_pools(gem_bs: &GemBS) -> Vec<Rc<String>> {
	let mut pools = Vec::new();
	if let Some(hr) = gem_bs.get_contig_hash().get(&ContigInfo::ContigPools) {
		for (key, _) in hr.iter() {
			pools.push(key.clone());
		}
	}
	pools
}

pub fn check_call(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Mapping, name ) { x } else { "." } };
	let bcf_dir = get_dir("bcf_dir").to_owned();
	let make_cram = gem_bs.get_config_bool(Section::Mapping, "make_cram");
	let ext = if make_cram { "cram" } else {"bam" };
	let samples = get_samples(gem_bs);
	let pools = get_contig_pools(gem_bs);
	let mut common_inputs = Vec::new();
	for f in &["gembs_reference", "gembs_reference_fai", "gembs_reference_gzi"] {
		if let Some(x) = gem_bs.get_asset(*f) { common_inputs.push(x.idx()) } else { panic!("{} not found", f) };
	}
	let handle_file = |nm: String, p: &Path| {
		let tpath = Path::new(nm.as_str());
		let path: PathBuf = [p, tpath].iter().collect();
		(nm, path)
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
				let mut out_vec = Vec::new();
				let (nm, path) = handle_file(format!("{}_{}.bcf", bcode, pool), bcf_path);
				out_vec.push(gem_bs.insert_asset(nm.as_str(), &path, AssetType::Derived));
				let (nm, path) = handle_file(format!("{}_{}.json", bcode, pool), bcf_path);
				out_vec.push(gem_bs.insert_asset(nm.as_str(), &path, AssetType::Derived));
				out_bcfs.push(out_vec[0]);
				let tv = out_vec.clone();
				let call_task = gem_bs.add_task(format!("bcf_call_{}_{}", bcode, pool).as_str(), 
					format!("Call BCFs for pool {}, barcode {}", pool, bcode).as_str(),
				 	Command::Call, format!("-b {} --pool {}", bcode, pool).as_str(), in_vec, out_vec);
				for id in tv.iter() { gem_bs.get_asset_mut(*id).unwrap().set_creator(call_task); }
			}
			let (nm, path) = handle_file(format!("{}.bcf", bcode), bcf_path);
			let out = gem_bs.insert_asset(nm.as_str(), &path, AssetType::Derived);
			let merge_task = gem_bs.add_task(format!("bcf_merge_{}", bcode).as_str(), 
					format!("Merge BCFs for barcode {}", bcode).as_str(),
				 	Command::MergeBcfs, format!("-b {}", bcode).as_str(), out_bcfs, vec!(out));
			gem_bs.get_asset_mut(out).unwrap().set_creator(merge_task);
		} else {
			let mut in_vec = common_inputs.clone();
			in_vec.push(bam);
			let (nm, path) = handle_file(format!("{}.bcf", bcode), bcf_path);
			let out = gem_bs.insert_asset(nm.as_str(), &path, AssetType::Derived);
			let call_task = gem_bs.add_task(format!("bcf_call_{}", bcode).as_str(), 
					format!("Call BCFs for barcode {}", bcode).as_str(),
				 	Command::Call, format!("-b {}", bcode).as_str(), in_vec, vec!(out));
			gem_bs.get_asset_mut(out).unwrap().set_creator(call_task);
		}
	}
	Ok(())
}
// Check requirements and presence of source and derived files for calling
// Make asset list for BAMs, BCFs etc. associated with calling
use std::path::{Path, PathBuf};
use crate::common::defs::{Section, DataValue, Command};
use crate::common::assets::{AssetType, GetAsset};
use crate::common::assets;
use super::GemBS;
use super::contig;

pub fn check_call(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Mapping, name ) { x } else { "." } };
	let bcf_dir = get_dir("bcf_dir").to_owned();
	let make_cram = gem_bs.get_config_bool(Section::Mapping, "make_cram");
	let (ext, idx_ext) = if make_cram { ("cram", "cram.crai") } else { ("bam", "bam.csi") };
	let samples = gem_bs.get_samples();
	let pools = contig::get_contig_pools(gem_bs);
	let mut common_inputs = Vec::new();
	for f in &["gembs_reference", "gembs_reference_fai", "gembs_reference_gzi"] {
		if let Some(x) = gem_bs.get_asset(*f) { common_inputs.push(x.idx()) } else { panic!("{} not found", f) };
	}
	if let Some(x) = gem_bs.get_asset("dbsnp_index") { common_inputs.push(x.idx()) } 
	let handle_file = |gb: &mut GemBS, nm: String, id: Option<String>, p: &Path| {
		let tpath = Path::new(nm.as_str());
		let path: PathBuf = [p, tpath].iter().collect();
		if let Some(s) = id { gb.insert_asset(s.as_str(), &path, AssetType::Derived) }
		else { gb.insert_asset(nm.as_str(), &path, AssetType::Derived) }
	}; 
	for (bcode, name) in samples.iter() {
		let bam = if let Some(x) = gem_bs.get_asset(format!("{}.{}", bcode, ext).as_str()) { x.idx() } 
		else { panic!("alignment file {}.{} not found", bcode, ext); };
		let bam_idx = if let Some(x) = gem_bs.get_asset(format!("{}.{}", bcode, idx_ext).as_str()) { x.idx() } 
		else { panic!("index file {}.{} not found", bcode, idx_ext); };
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
				in_vec.push(bam_idx);
				let out = handle_file(gem_bs, format!("{}_{}.bcf", bcode, pool), None, bcf_path);
				let out1 = handle_file(gem_bs, format!("{}_{}.json", bcode, pool), Some(format!("{}_{}_call.json", bcode, pool)), bcf_path);
				out_bcfs.push(out);
				let id = format!("bcf_call_{}_{}", bcode, pool);
				let (lname, lpath) = assets::make_ext_asset(&id, bcf_path, "log");
				let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
				let call_task = gem_bs.add_task(&id, format!("Call BCFs for pool {}, barcode {}", pool, bcode).as_str(),
				 	Command::Call, format!("--barcode {} --pool {}", bcode, pool).as_str(), &in_vec, &[out, out1], Some(log_index));
				[out, out1].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(call_task, &in_vec));
			}
			let out = handle_file(gem_bs, format!("{}.bcf", bcode), None, bcf_path);
			let id = format!("bcf_mergecall_{}", bcode);
			let (lname, lpath) = assets::make_ext_asset(&id, bcf_path, "log");
			let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
			let merge_task = gem_bs.add_task(&id, format!("Merge BCFs for barcode {}", bcode).as_str(),
				 	Command::MergeBcfs, format!("--barcode {} --no_md5 --no_index", bcode).as_str(), &out_bcfs, &[out], Some(log_index));
			gem_bs.get_asset_mut(out).unwrap().set_creator(merge_task, &out_bcfs);
		} else {
			let mut in_vec = common_inputs.clone();
			in_vec.push(bam);
			let out = handle_file(gem_bs, format!("{}.bcf", bcode), None, bcf_path);
			let out1 = handle_file(gem_bs, format!("{}.json", bcode), Some(format!("{}_call.json", bcode)), bcf_path);
			let id = format!("bcf_call_{}", bcode);
			let (lname, lpath) = assets::make_ext_asset(&id, bcf_path, "log");
			let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
			let call_task = gem_bs.add_task(&id, format!("Call BCFs for barcode {}", bcode).as_str(),
				 	Command::Call, format!("--barcode {} --no_md5 --no_index", bcode).as_str(), &in_vec, &[out, out1], Some(log_index));
			[out, out1].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(call_task, &in_vec));
		}
		let id = format!("{}.bcf", bcode);
		let bcf_asset = gem_bs.get_asset(id.as_str()).expect("Couldn't find bcf asset").idx();

		// Add md5sum asset and task
		let (md5_name, md5_path)  = assets::make_ext_asset(gem_bs.get_asset(bcf_asset).unwrap().id(), &bcf_path, "md5");
		let md5 = gem_bs.insert_asset(&md5_name, &md5_path, AssetType::Derived);
		let md5_task = gem_bs.add_task(&md5_name, format!("Calc MD5 sum for {}", id).as_str(),
			Command::MD5Sum, format!("--barcode {} call", bcode).as_str(), &[bcf_asset], &[md5], None);
		gem_bs.get_asset_mut(md5).unwrap().set_creator(md5_task, &[bcf_asset]);

		// Add bcf-index asset and task
		let (csi_name, csi_path)  = assets::make_ext_asset(gem_bs.get_asset(bcf_asset).unwrap().id(), &bcf_path, "csi");
		let csi = gem_bs.insert_asset(&csi_name, &csi_path, AssetType::Derived);
		let csi_task = gem_bs.add_task(&csi_name, format!("Calc Index for {}", id).as_str(),
			Command::IndexBcf, format!("--barcode {}", bcode).as_str(), &[bcf_asset], &[csi], None);
		gem_bs.get_asset_mut(csi).unwrap().set_creator(csi_task, &[bcf_asset]);		
	}
	Ok(())
}
// Check requirements and presence of source and derived files for calling
// Make asset list for BAMs, BCFs etc. associated with calling
use std::path::{Path, PathBuf};
use crate::common::defs::{Section, DataValue, Command};
use crate::common::assets::{AssetType, GetAsset};
use crate::common::assets;
use super::GemBS;
use super::contig;

pub fn check_call(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Calling, name ) { x } else { "." } };
	let bcf_dir = get_dir("bcf_dir").to_owned();
	let make_cram = gem_bs.get_config_bool(Section::Mapping, "make_cram");
	let (ext, idx_ext) = if make_cram { ("cram", "cram.crai") } else { ("bam", "bam.csi") };
	let cores = gem_bs.get_config_int(Section::Calling, "cores").map(|x| x as usize).or_else(|| Some(2));
	let merge_cores = gem_bs.get_config_int(Section::Calling, "merge_cores").map(|x| x as usize).or(cores);
	let memory = gem_bs.get_config_memsize(Section::Calling, "memory").or_else(|| Some(0x100000000.into())); // 4G
	let merge_memory = gem_bs.get_config_memsize(Section::Calling, "merge_memory").or(memory);
	let time = gem_bs.get_config_joblen(Section::Calling, "time").or_else(|| Some(3600.into()));
	let merge_time = gem_bs.get_config_joblen(Section::Calling, "time").or(time);
	let samples = gem_bs.get_samples();
	let pools = contig::get_contig_pools(gem_bs);
	let mut common_inputs = Vec::new();
	for f in &["gembs_reference", "gembs_reference_fai", "gembs_reference_gzi"] {
		if let Some(x) = gem_bs.get_asset(*f) { common_inputs.push(x.idx()) } else { panic!("{} not found", f) };
	}
	if let Some(x) = gem_bs.get_asset("dbsnp_index") { common_inputs.push(x.idx()) } 
	let handle_file = |gb: &mut GemBS, nm: String, id: Option<String>, p: &Path, st: AssetType| {
		let tpath = Path::new(nm.as_str());
		let path: PathBuf = [p, tpath].iter().collect();
		if let Some(s) = id { gb.insert_asset(s.as_str(), &path, st) }
		else { gb.insert_asset(nm.as_str(), &path, st) }
	}; 
	for (bcode, name) in samples.iter() {
		let bam = if let Some(x) = gem_bs.get_asset(format!("{}.{}", bcode, ext).as_str()) { x.idx() } 
		else { panic!("alignment file {}.{} not found", bcode, ext); };
		let bam_idx = if let Some(x) = gem_bs.get_asset(format!("{}.{}", bcode, idx_ext).as_str()) { x.idx() } 
		else { panic!("index file {}.{} not found", bcode, idx_ext); };
		let bam_md5 = if let Some(x) = gem_bs.get_asset(format!("{}.{}.md5", bcode, ext).as_str()) { x.idx() } 
		else { panic!("alignment md5 file {}.{}.md5 not found", bcode, ext); };
		let replace_meta_var = |s: &str| {
			if let Some(sm) = name { s.replace("@BARCODE", bcode).replace("@SAMPLE", sm) } else { s.replace("@BARCODE", bcode) }
		};
		let tmp_bcf_dir = replace_meta_var(&bcf_dir);
		let bcf_path = Path::new(&tmp_bcf_dir);	
		if pools.len() > 1 {
			let mut out_bcfs = Vec::new();
			let mut out_jsons = Vec::new();
			for pool in pools.iter() {
				let mut in_vec = common_inputs.clone();
				in_vec.push(bam);
				in_vec.push(bam_idx);
				in_vec.push(bam_md5);
				let out = handle_file(gem_bs, format!("{}_{}.bcf", bcode, pool), None, bcf_path, AssetType::Temp);
				let out1 = handle_file(gem_bs, format!("{}_{}.json", bcode, pool), Some(format!("{}_{}_call.json", bcode, pool)), bcf_path, AssetType::Temp);
				out_bcfs.push(out);
				out_jsons.push(out1);
				let id = format!("bcf_call_{}_{}", bcode, pool);
				let (lname, lpath) = assets::make_ext_asset(&id, bcf_path, "log");
				let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
				let call_task = gem_bs.add_task(&id, format!("Call BCFs for pool {}, barcode {}", pool, bcode).as_str(),
				 	Command::Call, format!("--barcode {} --pool {}", bcode, pool).as_str());
				gem_bs.add_task_inputs(call_task, &in_vec).add_outputs(&[out, out1]).set_log(Some(log_index)).set_barcode(bcode)
					.add_cores(cores).add_memory(memory).add_time(time);
				[out, out1].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(call_task, &in_vec));
			}
			let out = handle_file(gem_bs, format!("{}.bcf", bcode), None, bcf_path, AssetType::Derived);
			let out1 = handle_file(gem_bs, format!("{}_call.json", bcode), None, bcf_path, AssetType::Derived);
			let id = format!("bcf_mergecall_{}", bcode);
			let (lname, lpath) = assets::make_ext_asset(&id, bcf_path, "log");
			let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
			let merge_task = gem_bs.add_task(&id, format!("Merge BCFs for barcode {}", bcode).as_str(),
				 	Command::MergeBcfs, format!("--barcode {} --no-md5 --no-index", bcode).as_str());
			gem_bs.add_task_inputs(merge_task, &out_bcfs).add_outputs(&[out]).set_log(Some(log_index)).set_barcode(bcode)
				.add_cores(merge_cores).add_memory(merge_memory).add_time(merge_time);
			gem_bs.get_asset_mut(out).unwrap().set_creator(merge_task, &out_bcfs);
			let id = format!("merge_call_jsons_{}", bcode);
			let merge_json_task = gem_bs.add_task(&id, format!("Merge JSONs for barcode {}", bcode).as_str(), Command::MergeCallJsons, "");
			gem_bs.add_task_inputs(merge_json_task, &out_jsons).add_outputs(&[out1]).set_barcode(bcode);
			gem_bs.get_asset_mut(out1).unwrap().set_creator(merge_json_task, &out_jsons);
		} else {
			let mut in_vec = common_inputs.clone();
			in_vec.push(bam);
			in_vec.push(bam_md5);
			let out = handle_file(gem_bs, format!("{}.bcf", bcode), None, bcf_path, AssetType::Derived);
			let out1 = handle_file(gem_bs, format!("{}.json", bcode), Some(format!("{}_call.json", bcode)), bcf_path, AssetType::Derived);
			let id = format!("single_bcf_call_{}", bcode);
			let (lname, lpath) = assets::make_ext_asset(&id, bcf_path, "log");
			let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
			let call_task = gem_bs.add_task(&id, format!("Call BCFs for barcode {}", bcode).as_str(),
				 	Command::Call, format!("--barcode {} --no-md5 --no-index", bcode).as_str());
			gem_bs.add_task_inputs(call_task, &in_vec).add_outputs(&[out, out1]).set_log(Some(log_index)).set_barcode(bcode)
				.add_cores(cores).add_memory(memory).add_time(time);
			[out, out1].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(call_task, &in_vec));
		}
		let id = format!("{}.bcf", bcode);
		let bcf_asset = gem_bs.get_asset(id.as_str()).expect("Couldn't find bcf asset").idx();

		// Add md5sum asset and task
		let (md5_name, md5_path)  = assets::make_ext_asset(gem_bs.get_asset(bcf_asset).unwrap().id(), &bcf_path, "md5");
		let md5 = gem_bs.insert_asset(&md5_name, &md5_path, AssetType::Derived);
		let md5_task = gem_bs.add_task(&md5_name, format!("Calc MD5 sum for {}", id).as_str(),
			Command::MD5SumCall, format!("--barcode {}", bcode).as_str());
		let md5_cores = gem_bs.get_config_int(Section::MD5Sum, "cores").map(|x| x as usize).or(Some(1));
		let md5_memory = gem_bs.get_config_memsize(Section::MD5Sum, "memory");
		let md5_time = gem_bs.get_config_joblen(Section::MD5Sum, "time").or_else(|| Some(3600.into()));
		gem_bs.add_task_inputs(md5_task, &[bcf_asset]).add_outputs(&[md5]).set_barcode(bcode)
			.add_cores(md5_cores).add_memory(md5_memory).add_time(md5_time);
		gem_bs.get_asset_mut(md5).unwrap().set_creator(md5_task, &[bcf_asset]);

		// Add bcf-index asset and task
		let (csi_name, csi_path)  = assets::make_ext_asset(gem_bs.get_asset(bcf_asset).unwrap().id(), &bcf_path, "csi");
		let csi = gem_bs.insert_asset(&csi_name, &csi_path, AssetType::Derived);
		let csi_task = gem_bs.add_task(&csi_name, format!("Calc Index for {}", id).as_str(),
			Command::IndexBcf, format!("--barcode {}", bcode).as_str());
		gem_bs.add_task_inputs(csi_task, &[bcf_asset]).add_outputs(&[csi]).set_barcode(bcode)
			.add_cores(cores).add_memory(memory).add_time(time);
		gem_bs.get_asset_mut(csi).unwrap().set_creator(csi_task, &[bcf_asset]);		
	}
	Ok(())
}
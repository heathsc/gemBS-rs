// Check requirements and presence of source and derived files for extraction
// Make asset list for BCFs, BED, BigWig etc. associated with traction

use std::path::{Path, PathBuf};
use crate::common::defs::{Section, DataValue, Command};
use crate::common::assets::{AssetType, GetAsset};
use crate::common::assets;
use super::GemBS;

pub fn check_extract(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Mapping, name ) { x } else { "." } };
	let extract_dir = get_dir("extract_dir").to_owned();
	let bw_strand_specific = gem_bs.get_config_bool(Section::Extract, "bigwig_strand_specific");
	let non_cpg = gem_bs.get_config_bool(Section::Extract, "make_non_cpg");
	let snps = gem_bs.get_config_bool(Section::Extract, "make_snps");
	let bedmethyl = gem_bs.get_config_bool(Section::Extract, "make_bedmethyl");
	let cpg = gem_bs.get_config_bool(Section::Extract, "make_cpg")  || !(non_cpg || bedmethyl || snps);
	let samples = gem_bs.get_samples();
	let mut snpxtr_inputs = Vec::new();
	if let Some(x) = gem_bs.get_asset("dbsnp_index") { snpxtr_inputs.push(x.idx()) } 
	let mut mextr_suff: Vec<&str> = Vec::new();
	let mut mextr_comm = String::new();
	if cpg { 
		["cpg.txt.gz", "cpg.txt.gz.tbi", "cpg.txt.gz.md5"].iter().for_each(|x| mextr_suff.push(x));
		mextr_comm.push_str(" --cpg");
	}
	if non_cpg {
		["non_cpg.txt.gz", "non_cpg.txt.gz.tbi", "non_cpg.txt.gz.md5"].iter().for_each(|x| mextr_suff.push(x));
		mextr_comm.push_str(" --non-cpg");
	}
	if bedmethyl { 
		["cpg.bed.gz", "cpg.bed.gz.md5", "cpg.bb", "cpg.bb.md5", 
		 "chg.bed.gz", "chg.bed.gz.md5", "chg.bb", "chg.bb.md5",
		 "chh.bed.gz", "chh.bed.gz.md5", "chh.bb", "chh.bb.md5",].iter().for_each(|x| mextr_suff.push(x));
		if bw_strand_specific { 
			["pos.bw", "pos.bw.md5", "neg.bw", "neg.bw.md5"].iter().for_each(|x| mextr_suff.push(x)); 
		} else { [".bw", ".bw.md5"].iter().for_each(|x| mextr_suff.push(x)); }
		mextr_comm.push_str(" --bed-methyl");
	}
	let mut snpxtr_suff: Vec<&str> = Vec::new();
	if snps { ["snps.txt.gz", "snps.txt.gz.md5", "snps.txt.gz.tbi"].iter().for_each(|x| snpxtr_suff.push(x)) }
	let snp_list = if let Some(DataValue::String(s)) = gem_bs.get_config(Section::Index, "snp_list") { Some(PathBuf::from(s)) } else { None };
	if let Some(p) = snp_list { snpxtr_inputs.push(gem_bs.insert_asset("snp_list", &p, AssetType::Supplied)); }
	let handle_file = |gb: &mut GemBS, nm: String, p: &Path| {
		let tpath = Path::new(nm.as_str());
		let path: PathBuf = [p, tpath].iter().collect();
		gb.insert_asset(nm.as_str(), &path, AssetType::Derived)
	}; 

	for (bc, name) in samples.iter() {
		let bcf = if let Some(x) = gem_bs.get_asset(format!("{}.bcf", bc).as_str()) { x.idx() } 
		else { panic!("alignment file {}.bcf not found", bc); };
		let bcf_ix = if let Some(x) = gem_bs.get_asset(format!("{}.bcf.csi", bc).as_str()) { x.idx() } 
		else { panic!("BCF index file {}.bcf.csi not found", bc); };
		let replace_meta_var = |s: &str| {
			if let Some(sm) = name { s.replace("@BARCODE", bc).replace("@SAMPLE", sm) } else { s.replace("@BARCODE", bc) }
		};
		let tmp_extract_dir = replace_meta_var(&extract_dir);
		let extract_path = Path::new(&tmp_extract_dir);	
		if !mextr_suff.is_empty() {
			let mut out_vec = Vec::new();
			for suff in mextr_suff.iter() { out_vec.push(handle_file(gem_bs, format!("{}_{}", bc, suff), extract_path))}
			let id = format!("mextr_{}", bc);
			let (lname, lpath) = assets::make_ext_asset(&id, extract_path, "log");
			let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
			let task = gem_bs.add_task(&id, format!("Extract methylation values for barcode {}", bc).as_str(),
					Command::Extract, format!("{} --barcode {}", mextr_comm, bc).as_str(), &[bcf, bcf_ix], &out_vec, Some(log_index));
			out_vec.iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &[bcf]));
		}		
		if !snpxtr_suff.is_empty() {
			let mut out_vec = Vec::new();
			for suff in snpxtr_suff.iter() { out_vec.push(handle_file(gem_bs, format!("{}_{}", bc, suff), extract_path))}
			let id = format!("snpxtr_{}", bc);
			let (lname, lpath) = assets::make_ext_asset(&id, extract_path, "log");
			let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
			let task = gem_bs.add_task(&id, format!("Extract SNPs for barcode {}", bc).as_str(),
					Command::Extract, format!("--snps --barcode {}", bc).as_str(), &[bcf, bcf_ix], &out_vec, Some(log_index));
			out_vec.iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &[bcf]));
		}		
	}
	Ok(())
}
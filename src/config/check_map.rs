// Check requirements and presence of source and derived files for mapping
// Make asset list for FASTQs, BAMs etc. associated with mapping
use std::path::Path;
use std::collections::HashMap;
use crate::common::defs::{Metadata, Section, DataValue, Command};
use crate::common::assets;
use crate::common::assets::{AssetType, GetAsset};
use super::GemBS;

struct TempSample<'a> {
	name: Option<&'a str>,
	datasets: Vec<&'a str>,
}

impl<'a> TempSample<'a> {
	fn check_name(&mut self, name: Option<&'a str>, barcode: &'a str) -> Result<(), String> {
		if let Some(name1) = name {
			match self.name {
				Some(name2) => if name1 != name2 { 
					return Err(format!("Inconsistent sample name for barcode {}: found {}, expected {}", barcode, name1, name2)) 
				},
				None => self.name = Some(name1),
			}
		}
		Ok(())
	}	
}

struct Sample {
	barcode: String,
	name: Option<String>,
	datasets: Vec<String>,
}

fn collect_samples(gem_bs: &GemBS) -> Result<Vec<Sample>, String> {
	let mut bc: HashMap<&str, TempSample> = HashMap::new();
	let href = gem_bs.get_sample_data_ref();
	for (dataset, href1) in href.iter() {
		if let Some(DataValue::String(bcode)) = href1.get(&Metadata::SampleBarcode) {
			let name = href1.get(&Metadata::SampleName).and_then(|x| {
				if let DataValue::String(s) = x {Some(s.as_str()) } else { None }
			});
			let sample = bc.entry(bcode).or_insert_with(|| TempSample{name, datasets: Vec::new()});
			sample.check_name(name, bcode)?;
			sample.datasets.push(dataset);
		} else { return Err(format!("No barcode associated with dataset {}", dataset)) }
	}		
	let mut samples = Vec::new();
	for (bcode, sample) in bc.iter() {
		let barcode = bcode.to_string();
		let name = sample.name.map(|x| x.to_string());
		let datasets = sample.datasets.iter().map(|x| x.to_string()).collect();
		samples.push(Sample{barcode, name, datasets});
	}
	Ok(samples)
}

pub fn check_map(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Mapping, name ) { x } else { "." } };
	let seq_dir = get_dir("sequence_dir").to_owned();
	let bam_dir = get_dir("bam_dir").to_owned();
	let make_cram = gem_bs.get_config_bool(Section::Mapping, "make_cram");
	let mut common_inputs = Vec::new();
	if make_cram {
			for f in &["gembs_reference", "gembs_reference_fai", "gembs_reference_gzi", "contig_md5"] {
			if let Some(x) = gem_bs.get_asset(*f) { common_inputs.push(x.idx()) } else { panic!("{} not found", f) };
		}
	}
	let index_id = gem_bs.get_asset("index").map(|x| x.idx());
	let nonbs_index_id = gem_bs.get_asset("nonbs_index").map(|x| x.idx());
	let suffix = if make_cram { ".cram" } else { ".bam" };
	
	// Get vector of samples and their associated datasets
	let samples = collect_samples(gem_bs)?;

	let handle_file = |gb: &mut GemBS, dt: &str, s: &str, nm: &str, p: &Path, asset: AssetType| {
		let tpath = Path::new(s);
		let path = if tpath.has_root() { tpath.to_owned() } else { [p, tpath].iter().collect()	};
		let name = format!("{}{}", dt, nm);
		gb.insert_asset(&name, &path, asset)
	}; 
	for sample in samples.iter() {
		let replace_meta_var = |s: &str| {
			if let Some(sm) = &sample.name {	s.replace("@BARCODE", &sample.barcode).replace("@SAMPLE", sm.as_str()) } else { s.replace("@BARCODE", &sample.barcode) }
		};
		let bdir = replace_meta_var(&bam_dir);
		let bpath = Path::new(&bdir);
		let sdir = replace_meta_var(&seq_dir);
		let spath = Path::new(&sdir);
		let mut bams = Vec::new();
		for dat in sample.datasets.iter() {
			let href = gem_bs.get_sample_data_ref();
			let dvec: Vec<(Metadata, DataValue)> = if let Some(dr) = href.get(dat) {
				dr.iter().map(|(md, val)| ((*md), val.clone())).collect()
			} else { panic!("Lost dataset information {}", dat) };
			let mut bisulfite = true;
			let mut in_vec = Vec::new();
			for(md, val) in dvec.iter() {
				let asset = AssetType::Supplied;
				match md {
					Metadata::FilePath => if let DataValue::String(s) = val { in_vec.push(handle_file(gem_bs, dat, s, "_read", &spath, asset)) },
					Metadata::FilePath1 => if let DataValue::String(s) = val { in_vec.push(handle_file(gem_bs, dat, s, "_read1", &spath, asset)) },
					Metadata::FilePath2 => if let DataValue::String(s) = val { in_vec.push(handle_file(gem_bs, dat, s, "_read2", &spath, asset)) },
					Metadata::Bisulfite => if let DataValue::Bool(x) = val { if !*x { bisulfite = false; }},
					_ => (),
				}
			}
			if in_vec.is_empty() { return Err(format!("No datafiles for dataset {}", dat)); }
			if bisulfite { in_vec.push(index_id.unwrap()); } else { in_vec.push(nonbs_index_id.unwrap()); }
			common_inputs.iter().for_each(|x| in_vec.push(*x));
			
			if sample.datasets.len() > 1 {
				let out1 = handle_file(gem_bs, dat, format!("{}.bam", dat).as_str(), ".bam", &bpath, AssetType::Temp); 
				let out2 = handle_file(gem_bs, dat, format!("{}.json", dat).as_str(), ".json", &bpath, AssetType::Derived);
				let id = format!("map_{}", dat);
				let (lname, lpath) = assets::make_log_asset(&id, &bpath);
				let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);
				let task = gem_bs.add_task(&id, format!("Map dataset {} for barcode {}", dat, sample.barcode).as_str(),
					Command::Map, format!("--dataset {}", dat).as_str(), &in_vec, &[out1, out2], Some(log_index));
				[out1, out2].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &in_vec));
				bams.push(out1);
			} else {
				let out1 = handle_file(gem_bs, &sample.barcode, format!("{}{}", sample.barcode, suffix).as_str(), suffix, &bpath, AssetType::Derived);
				let out2 = handle_file(gem_bs, &sample.barcode, format!("{}.json", sample.barcode).as_str(), ".json", &bpath, AssetType::Derived);
				let id = format!("single_map_{}", sample.barcode);
				let (lname, lpath) = assets::make_log_asset(&id, &bpath);
				let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
				let task = gem_bs.add_task(&id, format!("Map dataset {} for barcode {}", dat, sample.barcode).as_str(),
					Command::Map, format!("--barcode {}", sample.barcode).as_str(), &in_vec, &[out1, out2], Some(log_index));
				[out1, out2].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &in_vec));
			};
		}
		if !bams.is_empty() {
			let out = handle_file(gem_bs, &sample.barcode, format!("{}{}", sample.barcode, suffix).as_str(), suffix, &bpath, AssetType::Temp);
			let id = format!("merge-bam_{}", sample.barcode);
			let (lname, lpath) = assets::make_log_asset(&id, &bpath);
			let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
			let task = gem_bs.add_task(&id, format!("Merge datasets for barcode {}", sample.barcode).as_str(),
				Command::MergeBams, format!("--barcode {}", sample.barcode).as_str(), &bams, &[out], Some(log_index));
			gem_bs.get_asset_mut(out).unwrap().set_creator(task, &bams);
		}
	}
	Ok(())
}


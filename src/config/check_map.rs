// Check requirements and presence of source and derived files for mapping
// Make asset list for FASTQs, BAMs etc. associated with mapping
use std::path::Path;
use std::collections::HashMap;
use glob::glob;
use regex::Regex;
use lazy_static::lazy_static;

use crate::common::defs::{Metadata, Section, DataValue, Command, FileType};
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

fn check_file_match(path: &Path) -> Option<Metadata> {
	lazy_static! { static ref RE: Regex = Regex::new(r"^.*?([12])?[.]fastq").unwrap(); }
	if let Some(s) = path.as_os_str().to_str() {
		if let Some(cap) = RE.captures(s) {
			if let Some(r) = cap.get(1) {
				if r.as_str() == "1" { Some(Metadata::FilePath1) } else { Some(Metadata::FilePath2) }
			} else { Some(Metadata::FilePath) }
		} else { None }
	} else { None } 
}

pub fn check_map(gem_bs: &mut GemBS) -> Result<(), String> {
	let get_dir = |name: &str| { if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Mapping, name ) { x } else { "." } };
	let seq_dir = get_dir("sequence_dir").to_owned();
	let bam_dir = get_dir("bam_dir").to_owned();
	let cores = gem_bs.get_config_int(Section::Mapping, "cores").map(|x| x as usize);
	let memory = gem_bs.get_config_memsize(Section::Mapping, "memory");
	let time = gem_bs.get_config_joblen(Section::Mapping, "time").or_else(|| Some(21600.into()));
	let make_cram = gem_bs.get_config_bool(Section::Mapping, "make_cram");
	let mut common_inputs = Vec::new();
	if make_cram {
			for f in &["gembs_reference", "gembs_reference_fai", "gembs_reference_gzi", "contig_md5"] {
			if let Some(x) = gem_bs.get_asset(*f) { common_inputs.push(x.idx()) } else { panic!("{} not found", f) };
		}
	}
	let index_id = gem_bs.get_asset("index").map(|x| x.idx());
	let nonbs_index_id = gem_bs.get_asset("nonbs_index").map(|x| x.idx());
	let (suffix, index_suff) = if make_cram { (".cram", ".cram.crai") } else { (".bam", ".bam.csi") };
	
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
			if let Some(sm) = &sample.name { s.replace("@BARCODE", &sample.barcode).replace("@SAMPLE", sm.as_str()) } else { s.replace("@BARCODE", &sample.barcode) }
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
			let mut alt_dataset = None;
			let mut file_type = None;
			for(md, val) in dvec.iter() {
				let asset = AssetType::Supplied;
				match md {
					Metadata::FilePath => if let DataValue::String(s) = val { in_vec.push(handle_file(gem_bs, dat, s, "_read", &spath, asset)) },
					Metadata::FilePath1 => if let DataValue::String(s) = val { in_vec.push(handle_file(gem_bs, dat, s, "_read1", &spath, asset)) },
					Metadata::FilePath2 => if let DataValue::String(s) = val { in_vec.push(handle_file(gem_bs, dat, s, "_read2", &spath, asset)) },
					Metadata::AltDataset => if let DataValue::String(s) = val { alt_dataset = Some(s) }
					Metadata::FileType => if let DataValue::FileType(t) = val { file_type = Some(t) }
					Metadata::Bisulfite => if let DataValue::Bool(x) = val { if !*x { bisulfite = false; }},
					_ => (),
				}
			}
			// If no data files specified, look for files based on Dataset or AltDataset
			if in_vec.is_empty() {
				let mut thash = HashMap::new();
				let mut dsets = vec!(dat);
				if let Some(d) = alt_dataset { dsets.push(d); }
				for dt in dsets {
					for mat in glob(format!("{}/*{}*fastq*", sdir, dt).as_str()).expect("Failed to read glob pattern") {
						match mat {
							Ok(p) => if let Some(md) = check_file_match(&p) { 
								if let Some(old_val) = thash.insert(md, p.clone()) {
									if old_val != p { return Err(format!("Inconsistent data for dataset {} item {:?}", dat, md)); }
								}
							},
							Err(e) => return Err(format!("Error when searching for datafiles: {}", e)),
						}
					}
					if !thash.is_empty() { break; }
				}
				// Check that the files we found are compatible iwth themselves and other information
				if thash.contains_key(&Metadata::FilePath1) || thash.contains_key(&Metadata::FilePath2) {
					if thash.contains_key(&Metadata::FilePath) { return Err(format!("Mixture of paired and single sequence files found for dataset {} in {}", dat, sdir)); }
					if thash.contains_key(&Metadata::FilePath1) && thash.contains_key(&Metadata::FilePath2) {
						if let Some(ft) = file_type {
							if *ft == FileType::Interleaved || *ft == FileType::Single {
								return Err(format!("Error with dataset {}: Interleaved or Single file type incompatible with multiple sequence files", dat)); 
							} 					
						} else { gem_bs.set_sample_data(dat, Metadata::FileType, DataValue::FileType(FileType::Paired)) }	
					}
				}
				// Everything OK, so insert assets
				for(md, p) in thash.iter() {
					let ext = match md {
						Metadata::FilePath => "read",
						Metadata::FilePath1 => "read1",
						Metadata::FilePath2 => "read2",
						_ => panic!("Unexpected metadata type"),
					};
					gem_bs.set_sample_data(dat, *md, DataValue::String(format!("{}", p.display())));
					in_vec.push(gem_bs.insert_asset(format!("{}_{}", dat, ext).as_str(), &p, AssetType::Supplied));		
				}
			}
			if in_vec.is_empty() { return Err(format!("No datafiles for dataset {}", dat)); }
			if bisulfite { in_vec.push(index_id.unwrap()); } else { in_vec.push(nonbs_index_id.unwrap()); }
			common_inputs.iter().for_each(|x| in_vec.push(*x));
			
			if sample.datasets.len() > 1 {
				let out1 = handle_file(gem_bs, dat, format!("{}.bam", dat).as_str(), ".bam", &bpath, AssetType::Temp); 
				let out2 = handle_file(gem_bs, dat, format!("{}.json", dat).as_str(), "_map.json", &bpath, AssetType::Derived);
				let id = format!("map_{}", dat);
				let (lname, lpath) = assets::make_ext_asset(&id, &bpath, "log");
				let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);
				let task = gem_bs.add_task(&id, format!("Map dataset {} for barcode {}", dat, sample.barcode).as_str(),
					Command::Map, format!("--dataset {}", dat).as_str());
				gem_bs.add_task_inputs(task, &in_vec).add_outputs(&[out1, out2]).set_log(Some(log_index)).set_barcode(&sample.barcode)
					.add_cores(cores).add_memory(memory).add_time(time);
				[out1, out2].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &in_vec));
				bams.push(out1);
			} else {
				let out1 = handle_file(gem_bs, &sample.barcode, format!("{}{}", sample.barcode, suffix).as_str(), suffix, &bpath, AssetType::Derived);
				let out2 = handle_file(gem_bs, &sample.barcode, format!("{}.json", sample.barcode).as_str(), "_map.json", &bpath, AssetType::Derived);
				let out3 = handle_file(gem_bs, &sample.barcode, format!("{}{}", sample.barcode, index_suff).as_str(), index_suff, &bpath, AssetType::Derived);
				let id = format!("single_map_{}", sample.barcode);
				let (lname, lpath) = assets::make_ext_asset(&id, &bpath, "log");
				let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
				let task = gem_bs.add_task(&id, format!("Map dataset {} for barcode {}", dat, sample.barcode).as_str(),
					Command::Map, format!("--barcode {} --no-md5", sample.barcode).as_str());
				gem_bs.add_task_inputs(task, &in_vec).add_outputs(&[out1, out2, out3]).set_log(Some(log_index)).set_barcode(&sample.barcode)
					.add_cores(cores).add_memory(memory).add_time(time);
				[out1, out2, out3].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &in_vec));
				let (md5_name, md5_path)  = assets::make_ext_asset(gem_bs.get_asset_mut(out1).unwrap().id(), &bpath, "md5");
				let md5 = gem_bs.insert_asset(&md5_name, &md5_path, AssetType::Derived);
				let md5_task = gem_bs.add_task(&md5_name, format!("Calc MD5 sum for {}", id).as_str(),
					Command::MD5Sum, format!("--barcode {} map", sample.barcode).as_str());
				let md5_cores = gem_bs.get_config_int(Section::MD5Sum, "cores").map(|x| x as usize).or(Some(1));
				let md5_memory = gem_bs.get_config_memsize(Section::MD5Sum, "memory");
				let md5_time = gem_bs.get_config_joblen(Section::MD5Sum, "time").or_else(|| Some(3600.into()));
				gem_bs.add_task_inputs(md5_task, &[out1]).add_outputs(&[md5]).set_barcode(&sample.barcode)
					.add_cores(md5_cores).add_memory(md5_memory).add_time(md5_time);
				gem_bs.get_asset_mut(md5).unwrap().set_creator(md5_task, &[out1]);
								
			};
		}
		if !bams.is_empty() {
			let out1 = handle_file(gem_bs, &sample.barcode, format!("{}{}", sample.barcode, suffix).as_str(), suffix, &bpath, AssetType::Temp);
			let out2 = handle_file(gem_bs, &sample.barcode, format!("{}{}", sample.barcode, index_suff).as_str(), index_suff, &bpath, AssetType::Derived);
			let id = format!("merge-bam_{}", sample.barcode);
			let (lname, lpath) = assets::make_ext_asset(&id, &bpath, "log");
			let log_index = gem_bs.insert_asset(&lname, &lpath, AssetType::Log);				
			let task = gem_bs.add_task(&id, format!("Merge datasets for barcode {}", sample.barcode).as_str(),
				Command::MergeBams, format!("--barcode {} --no-md5", sample.barcode).as_str());
			gem_bs.add_task_inputs(task, &bams).add_outputs(&[out1, out2]).set_log(Some(log_index)).set_barcode(&sample.barcode)
				.add_cores(cores).add_memory(memory).add_time(time);
			[out1, out2].iter().for_each(|id| gem_bs.get_asset_mut(*id).unwrap().set_creator(task, &bams));
			let out_asset = gem_bs.get_asset_mut(out1).unwrap();
			let (md5_name, md5_path)  = assets::make_ext_asset(out_asset.id(), &bpath, "md5");
			let md5 = gem_bs.insert_asset(&md5_name, &md5_path, AssetType::Derived);
			let md5_cores = gem_bs.get_config_int(Section::MD5Sum, "cores").map(|x| x as usize).or(Some(1));
			let md5_memory = gem_bs.get_config_memsize(Section::MD5Sum, "memory");
			let md5_time = gem_bs.get_config_joblen(Section::MD5Sum, "time").or_else(|| Some(3600.into()));
			let md5_task = gem_bs.add_task(&md5_name, format!("Calc MD5 sum for {}", id).as_str(),
				Command::MD5Sum, format!("--barcode {} map", sample.barcode).as_str());
			gem_bs.add_task_inputs(md5_task, &[out1]).add_outputs(&[md5]).set_barcode(&sample.barcode)
				.add_cores(md5_cores).add_memory(md5_memory).add_time(md5_time);
			gem_bs.get_asset_mut(md5).unwrap().set_creator(md5_task, &[out1]);
		}
	}
	Ok(())
}


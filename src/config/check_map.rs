// Check requirements and presence of source and derived files for mapping
// Make asset list for FASTQs, BAMs etc. associated with mapping
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::rc::Rc;
use crate::common::defs::{Metadata, Section, DataValue, Command};
use crate::common::assets::{Asset, AssetType};
use crate::common::tasks::{Task, TaskList};
use super::GemBS;

#[derive(Debug)]
struct Sample<'a> {
	name: Option<&'a str>,
	datasets: Vec<&'a str>,
}

impl<'a> Sample<'a> {
	fn check_name(&mut self, name: Option<&'a str>, barcode: &'a str) -> Result<(), String> {
		if let Some(name1) = name {
			match self.name {
				Some(name2) => if name1 != name2 { 
					return Err(format!("Inconsistent sample name for barcode {}: found {}, expected {}", barcode, name1, name2)) 
				},
				None => self.name = name,
			}
		}
		Ok(())
	}	
}

pub fn check_map(gem_bs: &mut GemBS) -> Result<(), String> {
	let seq_dir = if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Mapping, "sequence_dir") { x } else { "." };
	let bam_dir = if let Some(DataValue::String(x)) = gem_bs.get_config(Section::Mapping, "bam_dir") { x } else { "." };
	// Build hashtable with sample barcodes and their associated datasets
	let mut barcodes: HashMap<&str, Sample> = HashMap::new();
	let href = gem_bs.get_sample_data_ref();
	for (dataset, href1) in href.iter() {
		if let Some(DataValue::String(bcode)) = href1.get(&Metadata::SampleBarcode) {
			let name = href1.get(&Metadata::SampleName).and_then(|x| {
				if let DataValue::String(s) = x {Some(s.as_str()) } else { None }
			});
			let sample = barcodes.entry(bcode).or_insert_with(|| Sample{name, datasets: Vec::new()});
			sample.check_name(name, bcode)?;
			sample.datasets.push(dataset);
		} else { return Err(format!("No barcode associated with dataset {}", dataset)) }
	}
//	let mut asset_vec = Vec::new();
	let mut task_vec = Vec::new();
	let handle_file = |dt: &str, s: &str, nm: &str, p: &Path, asset: AssetType| {
		let tpath = Path::new(s);
		let path = if tpath.has_root() { tpath.to_owned() } else { [p, tpath].iter().collect()	};
		let name = format!("{}{}", dt, nm);
		(name, path, asset)
	};
	for (bcode, sample) in barcodes.iter() {
		let replace_meta_var = |s: &str| {
			if let Some(sm) = sample.name {	s.replace("@BARCODE", bcode).replace("@SAMPLE", sm)	} else { s.replace("@BARCODE", bcode) }
		};
		let bdir = replace_meta_var(bam_dir);
		let bpath = Path::new(&bdir);
//		asset_vec.push(handle_file(bcode, format!("{}.bam", bcode).as_str(), ".bam", &bpath, AssetType::Derived));
		let sdir = replace_meta_var(seq_dir);
		let spath = Path::new(&sdir);
//		if sample.datasets.len() > 1 {
//			let merge = (format!("merge-bam_{}", bcode), format!("Merge datasetsfor barcode {}", bcode),
//				Command::MergeBams, format!("-b {}", bcode));
//		};
		let mut tvec = Vec::new();
		for dat in sample.datasets.iter() {
			if let Some(dr) = href.get(*dat) {
				let mut in_vec = Vec::new();
				for(md, val) in dr.iter() {
					let asset = AssetType::Supplied;
					match md {
						Metadata::FilePath => if let DataValue::String(s) = val { in_vec.push(handle_file(dat, s, "_read", &spath, asset)); },
						Metadata::FilePath1 => if let DataValue::String(s) = val { in_vec.push(handle_file(dat, s, "_read1", &spath, asset)); },
						Metadata::FilePath2 => if let DataValue::String(s) = val { in_vec.push(handle_file(dat, s, "_read2", &spath, asset)); },
						_ => (),
					}
				}
				let mut out_vec = Vec::new();
				let map_dataset = if sample.datasets.len() == 1 {
					out_vec.push(handle_file(bcode, format!("{}.bam", bcode).as_str(), ".bam", &bpath, AssetType::Derived));
					out_vec.push(handle_file(bcode, format!("{}.json", bcode).as_str(), ".json", &bpath, AssetType::Derived));
					(format!("map_{}", bcode), format!("Map single dataset {} for barcode {}", sample.datasets[0], bcode),
						Command::Map, format!("-b {}", bcode))
				} else {
					out_vec.push(handle_file(dat, format!("{}.bam", dat).as_str(), ".bam", &bpath, AssetType::Temp));
					out_vec.push(handle_file(dat, format!("{}.json", dat).as_str(), ".json", &bpath, AssetType::Derived));
					(format!("map_{}", dat), format!("Map single dataset {} for barcode {}", sample.datasets[0], bcode),
						Command::Map, format!("-D {}", dat))
				};
				tvec.push((map_dataset, in_vec, out_vec));
			} else { panic!("Lost dataset information {}", dat) }
		}
		let bcode = (*bcode).to_string();
		task_vec.push((bcode, bpath.to_owned(), tvec));
	}
	for(bcode, bpath, tvec) in task_vec.iter() {
		let mut jobs = Vec::new();
		let mut out_bams = Vec::new();
		for ((id, desc, command, args), in_vec, out_vec) in tvec.iter() {
			let mut v1 = Vec::new();
			for (name, path, atype) in in_vec.iter() { v1.push(gem_bs.insert_asset(name.as_str(), &path, *atype)); }
			let mut v2 = Vec::new();
			for (name, path, atype) in out_vec.iter() { v2.push(gem_bs.insert_asset(name.as_str(), &path, *atype)); }
			out_bams.push(v2[0]);
			let map_task = gem_bs.add_task(id, desc, *command, args, v1, v2);
			jobs.push(map_task);
		}
		if jobs.len() > 1 {
			let (id, desc, command, args) = (format!("merge-bam_{}", bcode), format!("Merge datasetsfor barcode {}", bcode),
				Command::MergeBams, format!("-b {}", bcode));
			let(name, path, atype) = handle_file(bcode, format!("{}.bam", bcode).as_str(), ".bam", &bpath, AssetType::Derived);
			let ix = gem_bs.insert_asset(name.as_str(), &path, atype);
			let merge_task = gem_bs.add_task(&id, &desc, command, &args, out_bams, vec!(ix));
			for parent in jobs.iter() {
				gem_bs.add_parent_child(merge_task, *parent);
			}
		}
	}
	gem_bs.list_tasks();
	Ok(())
}
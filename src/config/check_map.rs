// Check requirements and presence of source and derived files for mapping
// Make asset list for FASTQs, BAMs etc. associated with mapping
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use crate::common::defs::{Metadata, Section, DataValue};
use crate::common::assets::{Asset, AssetType};
use crate::config::GemBS;

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
	let mut tvec = Vec::new();
	
	for (bcode, sample) in barcodes.iter() {
		let replace_meta_var = |s: &str| {
			if let Some(sm) = sample.name {	s.replace("@BARCODE", bcode).replace("@SAMPLE", sm)	} else { s.replace("@BARCODE", bcode) }
		};
		let handle_file = |dt: &str, s: &str, nm: &str, p: &Path, asset: AssetType| {
			let tpath = Path::new(s);
			let path = if tpath.has_root() { tpath.to_owned() } else { [p, tpath].iter().collect()	};
			let name = format!("{}{}", dt, nm);
			(name, path, asset)
		};
		let bdir = replace_meta_var(bam_dir);
		let bpath = Path::new(&bdir);
		tvec.push(handle_file(bcode, format!("{}.bam", bcode).as_str(), ".bam", &bpath, AssetType::Derived));
		if sample.datasets.len() == 1 {
			tvec.push(handle_file(bcode, format!("{}.json", bcode).as_str(), ".json", &bpath, AssetType::Derived));
		}
		let sdir = replace_meta_var(seq_dir);
		let spath = Path::new(&sdir);
		for dat in sample.datasets.iter() {
			if let Some(dr) = href.get(*dat) {
				for(md, val) in dr.iter() {
					let asset = AssetType::Supplied;
					match md {
						Metadata::FilePath => if let DataValue::String(s) = val { tvec.push(handle_file(dat, s, "_read", &spath, asset)); },
						Metadata::FilePath1 => if let DataValue::String(s) = val { tvec.push(handle_file(dat, s, "_read1", &spath, asset)); },
						Metadata::FilePath2 => if let DataValue::String(s) = val { tvec.push(handle_file(dat, s, "_read2", &spath, asset)); },
						_ => (),
					}
				}
				if sample.datasets.len() > 1 {
					tvec.push(handle_file(dat, format!("{}.bam", dat).as_str(), ".bam", &bpath, AssetType::Temp));
					tvec.push(handle_file(dat, format!("{}.json", dat).as_str(), ".json", &bpath, AssetType::Derived));
				}
			} else { panic!("Lost dataset information {}", dat) }
		}
	}
	for (name, path, atype) in tvec.iter() {
		gem_bs.insert_asset(Asset::new(name.as_str(), &path, *atype));		
	}
	Ok(())
}
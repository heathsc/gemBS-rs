use std::collections::HashMap;
use serde_json::{Value, Map};
use lazy_static::lazy_static;

use crate::config::GemBS;
use crate::common::defs::{Metadata, DataValue, ReadEnd, FileType};

pub mod process_csv;
pub mod process_json;

#[derive(Debug, Clone)]
struct SampleData {
	data: HashMap<String, HashMap<Metadata, DataValue>>,
}

impl SampleData {
	pub fn new() -> Self { SampleData{data: HashMap::new() }}
	
	pub fn add_csv_record(&mut self, hr: &mut HashMap<Metadata, DataValue>) -> Result<(), String> {
		if let Some(ds) = hr.remove(&Metadata::Dataset) {
			if let DataValue::String(data_set) = ds { self.check_and_store_record(hr, data_set)?; }
			else { return Err("Internal error".to_string()); }	
		} else { return Err("Data record missing Dataset information".to_string()); }
		Ok(())
	}
	
	pub fn add_json_record(&mut self, hr: &Map<String, Value>, dataset: String, alt_dataset: String) -> Result<(), String> {
		let mut thash = HashMap::new();
		thash.insert(Metadata::AltDataset, DataValue::String(alt_dataset));
		for(key, val) in hr.iter() {
			let kval = match key.as_str() {
				"sample_barcode" => if let Some(s) = val.as_str() { Some((Metadata::SampleBarcode, DataValue::String(s.to_string()))) } else { None },
				"library_barcode" => if let Some(s) = val.as_str() { Some((Metadata::LibraryBarcode, DataValue::String(s.to_string()))) } else { None },
				"sample_name" => if let Some(s) = val.as_str() { Some((Metadata::SampleName, DataValue::String(s.to_string()))) } else { None },
				"platform" => if let Some(s) = val.as_str() { Some((Metadata::Platform, DataValue::String(s.to_string()))) } else { None },
				"centre" => if let Some(s) = val.as_str() { Some((Metadata::Centre, DataValue::String(s.to_string()))) } else { None },
				"application" => if let Some(s) = val.as_str() { Some((Metadata::Bisulfite, DataValue::Bool(BISULFITE_APPS.contains(&(s.to_lowercase().as_str()))))) } else { None },
				_ => None,
			};
			if let Some((md, dval)) = kval { thash.insert(md, dval); }
		}
		self.check_and_store_record(&mut thash, dataset)	
	}
	
	fn check_and_store_record(&mut self, hr: &mut HashMap<Metadata, DataValue>, dataset: String) -> Result<(), String> {
		let thash = self.data.entry(dataset.clone()).or_insert_with(HashMap::new);
		if hr.contains_key(&Metadata::FilePath) {
			if let Some(end) = hr.remove(&Metadata::ReadEnd) {
				let fp = hr.remove(&Metadata::FilePath).unwrap();
				match end {
					DataValue::ReadEnd(end) if end == ReadEnd::End1 => hr.insert(Metadata::FilePath1, fp),
					DataValue::ReadEnd(end) if end == ReadEnd::End2 => hr.insert(Metadata::FilePath2, fp),
					_ => return Err("Internal error in check_and_store_record()".to_string()),
				};
			}
		}
		for (md, val) in hr.drain() {
			if let Some(old_val) = thash.insert(md, val.clone()) {
				if old_val != val { return Err(format!("Inconsistent data for dataset {} item {:?}", &dataset, md)); }
			}
		}
		Ok(())
	}
	
	// Sanity checking of the metadata that has been read in.
	pub fn check_and_store_sample_data(&mut self, gem_bs: &mut GemBS) -> Result<(), String> {
		for (dataset, href) in self.data.iter_mut() {
			// Check for paired data
			if href.contains_key(&Metadata::FilePath1) && href.contains_key(&Metadata::FilePath2) {
				if let Some(ftype) = href.get(&Metadata::FileType) {
					if let DataValue::FileType(ft) = ftype {
						if *ft == FileType::Interleaved || *ft == FileType::Single {
							return Err(format!("Error with dataset {}: Interleaved or Single file type incompatible with two input files", dataset)); 
						} 
					}
				} else { href.insert(Metadata::FileType, DataValue::FileType(FileType::Paired)); }
			}
			for (md, val) in href.iter() {
				gem_bs.set_sample_data(dataset, *md, val.clone());
			}
		}
		Ok(())
	}
}

lazy_static! {
    static ref BISULFITE_APPS: Vec<&'static str> = {
        let mut m = Vec::new();
		m.push("wg-bs-seq");
		m.push("bsseq");
		m.push("oxbs-seq");
		m.push("customcapturebs-seq");
		m.push("wg-em-seq");
		m.push("other-bs");
        m
    };
}

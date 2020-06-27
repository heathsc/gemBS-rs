use std::collections::{HashMap, HashSet};
use crate::config::GemBS;
use crate::common::defs::{DataValue, Metadata};

pub mod prepare;
pub mod index;
pub mod map;
pub mod call;
pub mod extract;
pub mod md5sum;
pub mod run;

pub fn get_barcode_list<'a>(gem_bs: &'a GemBS, options: &'a HashMap<&'static str, DataValue>) -> Result<Vec<&'a String>, String> {
	let mut barcodes = Vec::new();
	if let Some(DataValue::String(barcode)) = options.get("_barcode") { barcodes.push(barcode); }
	else if let Some(DataValue::String(sample)) = options.get("_sample") {
		for hr in gem_bs.get_sample_data_ref().values() {
			if let Some(DataValue::String(x)) = hr.get(&Metadata::SampleName) {
				if x == sample {
					if let Some(DataValue::String(barcode)) = hr.get(&Metadata::SampleBarcode) {	
						barcodes.push(barcode);
						break;
					}
				}
			}
		}
		if barcodes.is_empty() { return Err(format!("Unknown sample {}", sample)) } 
	} else {
		let mut samples = HashSet::new();
		for hr in gem_bs.get_sample_data_ref().values() {
			if let Some(DataValue::String(bc)) = hr.get(&Metadata::SampleBarcode) { samples.insert(bc); }
		}
		samples.iter().for_each(|x| barcodes.push(x));
	}
	Ok(barcodes)	
}

use crate::common::defs::{DataValue, Metadata};
use crate::config::GemBS;
use std::collections::{HashMap, HashSet};

pub mod call;
pub mod clear;
pub mod extract;
pub mod index;
pub mod map;
pub mod md5sum;
pub mod prepare;
pub mod report;
pub mod run;

pub fn get_barcode_list<'a>(
    gem_bs: &'a GemBS,
    options: &'a HashMap<&'static str, (DataValue, bool)>,
) -> Result<Vec<&'a String>, String> {
    let mut barcodes = Vec::new();
    if let Some((DataValue::StringVec(bvec), _)) = options.get("barcode") {
        barcodes.extend(bvec.iter());
    } else if let Some((DataValue::StringVec(svec), _)) = options.get("sample") {
        let mut seen = HashSet::new();
        for hr in gem_bs.get_sample_data_ref().values() {
            if let Some(DataValue::String(x)) = hr.get(&Metadata::SampleName) {
                if svec.contains(x) {
                    seen.insert(x);
                    if let Some(DataValue::String(barcode)) = hr.get(&Metadata::SampleBarcode) {
                        barcodes.push(barcode);
                    }
                }
            }
        }
        for sample in svec.iter() {
            if !seen.contains(sample) {
                return Err(format!("Unknown sample {}", sample));
            }
        }
    } else {
        let mut samples = HashSet::new();
        for hr in gem_bs.get_sample_data_ref().values() {
            if let Some(DataValue::String(bc)) = hr.get(&Metadata::SampleBarcode) {
                samples.insert(bc);
            }
        }
        samples.iter().for_each(|x| barcodes.push(x));
    }
    Ok(barcodes)
}

use std::str::FromStr;
use std::collections::HashMap;
use csv::{Reader, StringRecord};
use lazy_static::lazy_static;
use crate::config::GemBS;
use crate::commands::prepare::metadata::SampleData;
use crate::common::defs::{Metadata, DataValue, VarType};

#[derive(Debug)]
struct Header {
	columns: HashMap<Metadata, usize>,
}

impl Header {
	fn from_record(rec: &StringRecord) -> Result<Self, String> {
		let mut header = Header{columns: HashMap::new()};
		let mut trec = rec.clone();
		trec.trim();
		for (ix, field) in trec.iter().enumerate() {
			match Metadata::from_str(field) {
				Ok(md) => {
					if let Some(x) = header.columns.get(&md) {
						return Err(format!("Column {} clashes with column {} in cvs metadata file", x + 1, ix + 1));
					} else { header.columns.insert(md, ix); }
				},
				Err(_) => {	warn!("Warning: column {} in cvs metadata file not recognized", field); }
			}
		}
		if header.columns.contains_key(&Metadata::SampleBarcode) && header.columns.contains_key(&Metadata::Dataset) { Ok(header) }
		else { Err("Error: CVS metadata file does not contain sample barcode and dataset columns".to_string()) }	
	}
}

struct InputLine {
	vals: HashMap<Metadata, DataValue>,
	mask: u32,
}

impl InputLine {
	fn new() -> Self {
		InputLine{vals:HashMap::new(), mask: 0}
	}	
	fn insert(&mut self, key: Metadata, val: DataValue) -> Result<(), &'static str> {
		match key {
			Metadata::FilePath => self.mask |= 1,
			Metadata::FilePath1 | Metadata::FilePath2 => self.mask |= 2,
			Metadata::ReadEnd => self.mask |= 4,
			_ => (),
		};
		if (self.mask & 3) == 3 { return Err("can not have FilePath in the same record as FilePath1 or FilePath2")}
		if (self.mask & 6) == 6 { return Err("can not have ReadEnd in the same record as FilePath1 or FilePath2")}
		self.vals.insert(key, val);
		Ok(())
	}
	fn get_hash(&mut self) -> &mut HashMap<Metadata, DataValue> { &mut self.vals }
}

lazy_static! {
    static ref MD_ASSOC: HashMap<Metadata, VarType> = {
        let mut m = HashMap::new();
        m.insert(Metadata::SampleBarcode, VarType::String);
        m.insert(Metadata::SampleName, VarType::String);
        m.insert(Metadata::LibraryBarcode, VarType::String);
        m.insert(Metadata::Dataset, VarType::String);
        m.insert(Metadata::AltDataset, VarType::String);
        m.insert(Metadata::FileType, VarType::FileType);
        m.insert(Metadata::FilePath, VarType::String);
        m.insert(Metadata::FilePath1, VarType::String);
        m.insert(Metadata::FilePath2, VarType::String);
        m.insert(Metadata::ReadEnd, VarType::ReadEnd);
        m.insert(Metadata::Description, VarType::String);
        m.insert(Metadata::Centre, VarType::String);
        m.insert(Metadata::Platform, VarType::String);
        m.insert(Metadata::Bisulfite, VarType::Bool);
        m
    };
}

pub fn process_cvs_metatdata_file(file_name: &str, gem_bs: &mut GemBS) -> Result<(), String> {
	let mut rdr = Reader::from_path(file_name).map_err(|e| format!("Error opening cvs metadata file {} for input: {}", file_name, e))?;
	let hrecord = rdr.headers().map_err(|e| format!("Error reading header line from cvs metadata file {} for input: {}", file_name, e))?;
	let header = Header::from_record(&hrecord)?;
	let mut sample_data = SampleData::new();
   	let mut record = StringRecord::new();
	let mut line = 2;
	loop {
		let mut input_line = InputLine::new();
		match rdr.read_record(&mut record) {
			Ok(tst) => {
				if !tst { break; }
				record.trim();
				for (md, ix) in header.columns.iter() {
					if let Some(field) = record.get(*ix) {
						if !field.is_empty() { 
							match DataValue::from_str(field, *MD_ASSOC.get(md).unwrap()) {
								Ok(val) => {
									if let Err(e) = input_line.insert(*md, val) {
										return Err(format!("Error reading from cvs metadata file {}, line {}: {}", file_name, line, e));
									}
								},
								Err(_) => return Err(format!("Error reading from cvs metadata file {}, line {}, col {}", file_name, line, ix + 1)),
							};
						}
					}
				}
				if let Err(e) = sample_data.add_csv_record(input_line.get_hash()) {
					return Err(format!("Error reading record at line {} of cvs metadata file {}: {}", line, file_name, e));
				}
			},
			Err(e) => return Err(format!("Error reading record at line {} of cvs metadata file {} for input: {}", line, file_name, e)),	
		}
		line += 1;
	}
	sample_data.check_and_store_sample_data(gem_bs)
}

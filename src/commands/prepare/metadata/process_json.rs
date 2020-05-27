use serde_json::{Value, Map};
use crate::config::GemBS;
use crate::commands::prepare::metadata::SampleData;

fn get_fli(href: &Map<String, Value>) -> Result<(String, String), String> {
	let fc = match href.get("flowcell_name") {
		Some(x) => x.as_str(),
		None => None,
	};
	let lane = match href.get("lane_number") {
		Some(x) => x.as_i64(),
		None => None,
	};
	let index = match href.get("index_name") {
		Some(x) => x.as_str(),
		None => None,
	};	
	if fc.is_some() && lane.is_some() && index.is_some() {
		let s1 = format!("{}_{}", fc.unwrap(), lane.unwrap());
		Ok((format!("{}_{}",s1,index.unwrap()), format!("{}_0",s1)))
	} else { Err("Missing FLI name in JSON metadata entry".to_string()) }
}

fn check_pass(href: &Map<String, Value>) -> bool {
	let chk = match href.get("passfail") {
		Some(x) => x.as_str(),
		None => None,
	};
	match chk {
		Some(s) if s == "pass" => true,
		_ => false,
	}
}
	
pub fn process_json_metadata_file(file_name: &str, gem_bs: &mut GemBS) -> Result<(), String> {
	let rdr = match compress::open_reader(file_name) {
		Ok(reader) => { reader },
		Err(_) => {	return Err(format!("Could not open JSON metadata file {}", file_name)); }
	};
	let json_data: Value  = match serde_json::from_reader(rdr) {
		Ok(x) => x,
		Err(_) => {	return Err(format!("Could not parse JSON metadata file {}", file_name)); }
	};
	let json_data = match json_data {
		Value::Object(data) => data,
		_ => return Err(format!("Unexpected structure in JSON metadata file {}", file_name)),
	};
	let obj_array = match json_data.get("objects") {
		Some(x) => {
			if let Value::Array(y) = x { Some(y) }
			else { None }
		},
		None => None,
	};
	let json_data = if let Some(x) = obj_array { x }
	else { return Err(format!("Couldn't find objects map in JSON metadata file {}", file_name)); };
	
	let mut sample_data = SampleData::new();
	for data in json_data.iter() {
		if let Value::Object(obj) = data {
			let fli = get_fli(obj)?;
			if !check_pass(obj) {
				warn!("FLI {} skipped as passfail not set to pass", fli.0); 
				continue; 
			}	
			sample_data.add_json_record(obj, fli.0, fli.1)?;
		}	
	}
	sample_data.check_and_store_sample_data(gem_bs)
}
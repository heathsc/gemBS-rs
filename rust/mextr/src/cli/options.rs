use std::io;
use std::collections::HashMap;
use clap::ArgMatches;

use r_htslib::*;
use super::cli_utils;
use crate::config::*;

pub const OPTS: [(&str, ConfVar);21] = [
	("cpgfile", ConfVar::String(None)),
	("noncpgfile", ConfVar::String(None)),
	("bed_methyl", ConfVar::String(None)),
	("bed_track_line", ConfVar::String(None)),
	("report_file", ConfVar::String(None)),
	("no_header", ConfVar::Bool(false)),
	("common_gt", ConfVar::Bool(false)),
	("reference_bias", ConfVar::Float(2.0)),
	("threads", ConfVar::Int(1)),
	("min_nc", ConfVar::Int(1)),
	("number", ConfVar::Int(1)),
	("inform", ConfVar::Int(1)),
	("threshold", ConfVar::Int(20)),
	("bq_threshold", ConfVar::Int(20)),
	("haploid", ConfVar::Bool(false)),
	("compress", ConfVar::Bool(false)),
	("md5", ConfVar::Bool(false)),
	("tabix", ConfVar::Bool(false)),
	("mode", ConfVar::Mode(Mode::Combined)),
	("bw_mode", ConfVar::Mode(Mode::Combined)),
	("select", ConfVar::Select(Select::Hom)),
];

pub fn handle_options(m: &ArgMatches) -> io::Result<(ConfHash, BcfSrs)> {
	
	let mut conf_hash: HashMap<&'static str, ConfVar> = HashMap::new();
	// Handle simple options
	for (opt, val) in OPTS.iter()  { 
		let x = cli_utils::get_option(m, opt, val.clone())?;
		trace!("Inserting config option {} with value {:?}", opt, x);
		conf_hash.insert(opt, x);
	}
	// Conversion rates
	let (under, over) = if let Some(v) = cli_utils::get_fvec(m, "conversion", 1.0e-8, 1.0 - 1.0e-8)? { (v[0], v[1]) }
	else { (0.01, 0.05) };
	conf_hash.insert(&"under_conversion", ConfVar::Float(under));
	conf_hash.insert(&"over_conversion", ConfVar::Float(over));	

	// Min Proportion
	let prop = if let Some(x) = cli_utils::get_f64(m, "prop", 0.0, 1.0)? { x } else { 0.0 };

	let mut chash = ConfHash::new(conf_hash);
	
	// Check threshold
	if chash.get_int("threshold") > 255 { chash.set("threshold", ConfVar::Int(255)) }
	
	// Check output names for .gz
	// If so, strip suffix and set compress option
	for var in &["cpgfile", "noncpgfile", "bed_methyl"] {
		let tmp = chash.get_str(var).and_then(|s| s.strip_suffix(".gz")).map(|s| s.to_owned());
		if let Some(s) = tmp { 
			chash.set(var, ConfVar::String(Some(s)));
			chash.set("compress", ConfVar::Bool(true));
		}
	}

	let mut sr = BcfSrs::new()?;
	let infile = m.value_of("input").expect("No input filename"); // This should not be allowed by Clap	
	let regions = {			
		if let Some(mut v) = m.values_of("regions").or_else(|| m.values_of("region_list")) {
			let s = v.next().unwrap().to_owned();
			Some((v.fold(s, |mut st, x| {st.push(','); st.push_str(x); st}), false))
		} else if let Some(s) = m.value_of("region_file") { Some((s.to_owned(), true))}
		else { None }
	};
	if let Some((reg, flag)) = regions { sr.set_regions(&reg, flag)? }
	sr.add_reader(infile)?;
	
	// Check sample numbers
	let hdr = sr.get_reader_hdr(0)?;
	let ns =hdr.nsamples();	
	if ns == 0 { return Err(new_err(format!("No samples in input file {}", infile)))}
	
	// Check minimum sample numer
	let mn = chash.get_int("number").min(ns);
	let mn = mn.max((prop * (ns as f64) + 0.5) as usize);
	chash.set("number", ConfVar::Int(mn));
	
	if m.is_present("bed_methyl") {
		if ns > 1 { return Err(new_err(format!("Input file {} has {} samples: bedMethyl output incompatible with multisample files", infile, ns))) } 
		// Get sample description from VCF header if possible, otherwise use sample name
		let quotes = ['\'', '\"'];
		let trim = |s: &str| s.trim_start_matches(&quotes[..]).trim_end_matches(&quotes[..]).to_owned();
		let mut sample_name = None;
		let mut sample_desc = None;
		for ix in 0..hdr.nhrec() {
			let hr = hdr.hrec(ix)?;
			if hr.get_type() == BCF_HL_STR && hr.key() == "bs_call_sample_info" && hr.find_key("ID").is_some() {
				sample_name = hr.find_key("SM").map(|s| trim(s));
				sample_desc = hr.find_key("DS").map(|s| trim(s));
				break;
			}
		}
		sample_name = sample_name.or_else(|| Some(hdr.sample_name(0).unwrap().to_owned()));
		chash.set("sample_desc", ConfVar::String(sample_desc.or_else(|| Some(sample_name.as_ref().unwrap().to_owned()))));
		chash.set("sample_name", ConfVar::String(sample_name));
	} else { chash.set("sample_desc", ConfVar::String(None)) }
	Ok((chash, sr))
}
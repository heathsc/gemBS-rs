use std::io;
use std::collections::{HashMap, HashSet};
use clap::ArgMatches;

use utils::compress;

use crate::config::*;
use super::cli_utils::*;

fn read_select_file(s: &str) -> io::Result<HashSet<String>> {
	let mut sel_set = HashSet::new();
	let mut rdr = compress::open_bufreader(s)?;
	debug!("Reading selected SNP list from {}", s);
	let mut buf = String::with_capacity(256);
	loop {
		buf.clear();
		let l = rdr.read_line(&mut buf)?;
		if l == 0 { break }
		if let Some(sname) = buf.split_ascii_whitespace().next() { sel_set.insert(sname.to_owned()); }
	}
	debug!("Read in {} unique SNP IDs", sel_set.len());
	Ok(sel_set)	
}

fn read_alias_file(s: &str) -> io::Result<HashMap<String, String>> {
	let mut smap = HashMap::new();
	let mut rdr = compress::open_bufreader(s)?;
	debug!("Reading chromosome aliases from {}", s);
	let mut buf = String::with_capacity(256);
	loop {
		buf.clear();
		let l = rdr.read_line(&mut buf)?;
		if l == 0 { break }
		let mut it = buf.split('\t');
		if let (Some(s1), Some(s2)) = (it.next(), it.next()) { smap.insert(s1.to_owned(), s2.to_owned()); }
	}
	debug!("Read in {} aliases", smap.len());
	Ok(smap)	
}

pub fn handle_options(m: &ArgMatches) -> io::Result<(Config, Box<[String]>)> {
	trace!("Handle command line options");
	let threads = get_arg_usize(m, "threads")?.unwrap_or_else(num_cpus::get);
	let output = get_arg_string(m, "output");
	let description = get_arg_string(m, "description");
	let input_type = get_arg_itype(m, "input_type")?;
	let maf_limit = get_arg_f64(m, "maf_limit")?;
	let chrom_alias = match m.value_of("chrom_alias") {
		Some(s) => read_alias_file(s)?,
		None => HashMap::new(),
	};
	let selected = match m.value_of("selected") {
		Some(s) => read_select_file(s)?,
		None => HashSet::new(),
	};
	let files: Vec<String> = match m.values_of("input") {
		Some(v) => v.map(|s| s.to_owned()).collect(),
		None => Vec::new(),
	};
	trace!("Finished handling command line options");
	Ok((Config::new(threads, maf_limit, output, description, input_type, chrom_alias, selected), files.into_boxed_slice()))
}

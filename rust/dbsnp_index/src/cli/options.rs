use std::io;
use std::fs::metadata;
use std::collections::{HashMap, HashSet};
use clap::ArgMatches;

use utils::compress;
use r_htslib::*;

use crate::config::*;
use super::cli_utils::*;

fn read_select_file(s: &str) -> io::Result<HashSet<String>> {
	let mut sel_set = HashSet::new();
	let mut rdr = compress::open_bufreader(s)?;
	info!("Reading selected SNP list from {}", s);
	let mut buf = String::with_capacity(256);
	loop {
		buf.clear();
		let l = rdr.read_line(&mut buf)?;
		if l == 0 { break }
		if let Some(sname) = buf.split_ascii_whitespace().next() { 
			if let Some(name) = sname.strip_prefix("rs") { sel_set.insert(name.to_owned()); }
			else { sel_set.insert(sname.to_owned()); } 
		}
	}
	info!("Read in {} unique SNP IDs", sel_set.len());
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
		if let Some(c) = it.next() {
			let ctg = c.trim();
			// Insert an alias to the conical contig name
			smap.insert(ctg.to_owned(), ctg.to_owned());
			for alias in it {
				// Insert each alias
				smap.insert(alias.trim().to_owned(), ctg.to_owned());
			}
		}
	}
	debug!("Read in {} aliases", smap.len());
	Ok(smap)	
}

fn check_for_indexed_vcf_file(file: &str, tf: &mut Vec<(DbInput, i64)>, chrom_alias: Option<&HashMap<String, String>>) -> bool {
	if let Ok(hfile) = HtsFile::new(file, "r") {
		if hfile.format().format() != htsExactFormat::Vcf || !hfile.test_bgzf() { return false }
		if let Ok(tbx) = Tbx::new(file) {
			if let Some(v) = tbx.seq_names() {
				if let Some(alias) = &chrom_alias {
					for (tid, c) in v.iter().enumerate() {
						if let Some(ctg) = alias.get(*c) { tf.push((DbInput::VcfContig(file.to_owned(), ctg.to_string(), tid as libc::c_int), 1))}
					}
				} else {
					for (tid, c) in v.iter().enumerate() { tf.push((DbInput::VcfContig(file.to_owned(), c.to_string(), tid as libc::c_int), 1))}
				}
				true
			} else { false }
		} else { false }
	} else { false }	
}

pub fn handle_options(m: &ArgMatches) -> io::Result<(Config, Box<[DbInput]>)> {
	trace!("Handle command line options");
	let threads = get_arg_usize(m, "threads")?.unwrap_or_else(num_cpus::get);
	let jobs = get_arg_usize(m, "jobs")?.unwrap_or(1);
	let output = get_arg_string(m, "output");
	let description = get_arg_string(m, "description");
	let input_type = get_arg_itype(m, "input_type")?;
	let maf_limit = get_arg_f64(m, "maf_limit")?;
	let chrom_alias = match m.get_one::<String>("chrom_alias") {
		Some(s) => Some(read_alias_file(s)?),
		None => None,
	};
	let selected = match m.get_one::<String>("selected") {
		Some(s) => read_select_file(s)?,
		None => HashSet::new(),
	};
	let hts_log_level = unsafe {
		let t = hts_get_log_level();
		hts_set_log_level(htsLogLevel::HTS_LOG_OFF);
		t
	};
	let files: Vec<DbInput> = match m.get_many::<String>("input") {
		Some(v) => { 
			// Sort input files by file size in reverse order so that larger files are processed first
			let mut tf: Vec<(DbInput, i64)> = Vec::with_capacity(v.len());
			for file in v {
				match metadata(file) {
					Ok(m) => {
						let indexed_vcf = match input_type {
							IType::Auto | IType::Vcf => check_for_indexed_vcf_file(file, &mut tf, chrom_alias.as_ref()),
							_ => false,
						};
						if !indexed_vcf { 
							tf.push((DbInput::File(file.to_owned()), m.len() as i64)); 
						}
					},
					Err(e) => {
						error!("Couldn't get information on input file {}: {}", file, e);
						return Err(e);
					},
				}
			}
			tf.sort_unstable_by_key(|(_, s)| -s);
			let f: Vec<_> = tf.drain(..).map(|(d, _)| d).collect();
			f
		},
		None => vec!(DbInput::File("-".to_string())),
	};
	unsafe { hts_set_log_level(hts_log_level) };
	trace!("Finished handling command line options");
	Ok((Config::new(threads, jobs, maf_limit, output, description, input_type, chrom_alias, selected), files.into_boxed_slice()))
}

use std::io;
use std::io::Write;
use std::thread;
use std::sync::{Arc, atomic::{Ordering, AtomicUsize}};

use crossbeam_channel::bounded;
use r_htslib::*;
use crate::config::*;
use crate::dbsnp::DBSnpContig;

pub fn process(mut conf: Config) -> io::Result<()> {
	// Get output filename
	let output_name = conf.output().filename().unwrap_or("-");
	// Open output file
	let output_mode = if conf.output().compress() { "wz" } else { "w" };
	let mut out = HtsFile::new(output_name, output_mode)?;
	if conf.threads() > 0 { out.set_threads(conf.threads())? }
	let mut dbsnp_file = conf.dbsnp_file(); 
	let sel_hash = conf.selected_hash();
	let sr = conf.synced_reader();
	let hdr = sr.get_reader_hdr(0)?;
	let ns = hdr.nsamples();
	assert!(ns > 0);	
	assert_eq!(Some(0), hdr.id2int(BCF_DT_ID as usize, "PASS"));
	
	let mut brec = BcfRec::new()?;
	let mut curr_ctg: Option<(usize, String, Option<DBSnpContig>)> = None;
	let mut mdb = MallocDataBlock::<i32>::new();
	let mut gt_string = String::with_capacity(2);
	while sr.next_line() > 0 {
		brec = sr.swap_line(0, brec)?;
		let changed = if let Some((rid, cname, dbsnp_ctg)) = &curr_ctg {
			if brec.rid() != *rid { 
				if dbsnp_ctg.is_some() {
					let dbf = dbsnp_file.as_mut().unwrap();
					dbf.unload_ctg(&cname); 
				}
				true
			} else { false }
		} else { true };
		if changed {
			let new_rid = brec.rid();
			let name = hdr.ctg_name(new_rid)?;
			info!("Processing contig {}", name);
			let dbsnp_ctg = if let Some(dbf) = &mut dbsnp_file { 
				if dbf.load_ctg(name).is_ok() { dbf.get_dbsnp_contig(name) } else { None }
			} else { None }; 
			curr_ctg = Some((new_rid, name.to_owned(), dbsnp_ctg))
		}
		let pos = brec.pos();
		let rs_id = {
			let rs = brec.id();
			if rs == "." {
				if let Some((_, _, Some(dbsnp_ctg))) = &curr_ctg {
					if let Some((s, _)) = dbsnp_ctg.lookup_rs(pos) { Some(s) }
					else { None }
				} else { None }
			} else { Some(rs.to_owned()) } 	
		};
		let mut pass = if let Some(rs) = &rs_id {
			if let Some(sh) = &sel_hash {
				if let Some(name) = rs.strip_prefix("rs") { sh.contains(name) }
				else { sh.contains(rs) }
			} else { true }
		} else { false };
		if pass { pass = brec.check_pass()};
		if !pass { continue; }
		let alls = {
			let v = brec.alleles();
			if v.len() > 4 || v.iter().any(|x| x.len() != 1) { continue; }
			let mut a: Vec<char> = Vec::with_capacity(v.len() + 1);
			a.push('0');
			for all in v { a.push(all.chars().next().unwrap())}	
			a
		};
		let get_gt = |x: i32| {
			let i = (x >> 1) as usize;
			if i >= alls.len() { '.' }
			else { alls[i] }
		};
		mdb = brec.get_genotypes(&hdr, mdb)?;
		if mdb.iter().any(|x| *x > 0) {
			let ploidy = mdb.len() / ns;
			if ploidy == 0 { continue }
			let ctg_name = &curr_ctg.as_ref().unwrap().1;
			write!(out, "{}\t{}\t{}", ctg_name, pos + 1, rs_id.unwrap())?;
			for gt in mdb.chunks(ploidy) {
				gt_string.clear();
				for i in gt { gt_string.push(get_gt(*i)) }		
				write!(out, "\t{}", gt_string)?;
			}
			writeln!(out)?;
		}
	}
	Ok(())
}

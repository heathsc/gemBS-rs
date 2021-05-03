use std::io::{self, Write};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use r_htslib::*;
use crate::config::*;
use crate::dbsnp::DBSnpContig;
use crate::md5::md5_thread;
use crate::tabix::tabix_thread;

struct Md5TabixProc<T> {
	s: Sender<bool>,
	th: thread::JoinHandle<T>,
}

pub fn process(mut conf: Config) -> io::Result<()> {
	let mut dbsnp_file = conf.dbsnp_file(); 
	let sel_hash = conf.selected_hash();
	let mut sr = conf.synced_reader().expect("Synced reader is not set");
	let hdr = sr.get_reader_hdr(0)?.dup();
	let ns = hdr.nsamples();
	assert!(ns > 0);	
	assert_eq!(Some(0), hdr.id2int(BCF_DT_ID as usize, "PASS"));
	// Get output filename
	let output_name = conf.output().filename().unwrap_or("-");
	// Open output file
	let output_mode = if conf.output().compress() { "wz" } else { "w" };
	let mut out= HtsFile::new(output_name, output_mode)?;
	if conf.threads() > 0 { out.set_threads(conf.threads())? }
	let mut brec = BcfRec::new()?;
	let mut curr_ctg: Option<(usize, String, Option<DBSnpContig>)> = None;
	let mut buffer: Vec<u8> = Vec::with_capacity(80);
    let mut mdb = MallocDataBlock::<i32>::new();
    let mut mc8 = MallocDataBlock::<i32>::new();
	let mut procs = Vec::new();
	if conf.output().compute_md5() {
		let (s, r) = channel();
		let name = output_name.to_owned();
		let th = thread::spawn(move || md5_thread(name, r));
		procs.push(Md5TabixProc{ s, th});
	} 
	if conf.output().compute_tbx() {
		let (s, r) = channel();
		let name = output_name.to_owned();
		let th = thread::spawn(move || tabix_thread(name, r));
		procs.push(Md5TabixProc{ s, th});
	}
	while sr.next_line() > 0 {
		sr.swap_line(0, &mut brec)?;
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
		if brec.get_genotypes(&hdr, &mut mdb).is_none() || brec.get_format_i32(&hdr, "MC8", &mut mc8).is_none() { continue }
		let alls = {
			let v = brec.alleles();
			if v.len() > 4 || v.iter().any(|x| x.len() != 1) { continue; }
			let mut a: Vec<u8> = Vec::with_capacity(v.len() + 1);
			a.push(b'0');
			for all in v { a.push(all.as_bytes()[0])}	
			a
		};
		let get_gt = |x: i32| {
			let i = (x >> 1) as usize;
			if i >= alls.len() { b'.' }
			else { alls[i] }
		};
        let safe_frac = |a: i32, b: i32| { if a + b > 0 { (a as f64) / ((a + b) as f64) } else { 0.0 } };
        let get_baf = |x1: i32, x2: i32| {
            match (get_gt(x1), get_gt(x2)) {
                (b'A', b'C') => safe_frac(mc8[1]+ mc8[5], mc8[0] + mc8[4]),
                (b'A', b'G') => safe_frac(mc8[2], mc8[0]),
                (b'A', b'T') => safe_frac(mc8[3]+ mc8[7], mc8[0] + mc8[4]),                    
                (b'C', b'A') => safe_frac(mc8[0]+ mc8[4], mc8[1] + mc8[5]),
                (b'C', b'G') => safe_frac(mc8[2]+ mc8[6], mc8[1] + mc8[5]),
                (b'C', b'T') => safe_frac(mc8[3], mc8[1]),
                (b'G', b'A') => safe_frac(mc8[0], mc8[2]),
                (b'G', b'C') => safe_frac(mc8[1]+ mc8[5], mc8[2] + mc8[6]),
                (b'G', b'T') => safe_frac(mc8[3]+ mc8[7], mc8[2] + mc8[6]),
                (b'T', b'A') => safe_frac(mc8[0]+ mc8[4], mc8[3] + mc8[7]),
                (b'T', b'C') => safe_frac(mc8[1], mc8[3]),
                (b'T', b'G') => safe_frac(mc8[2]+ mc8[6], mc8[3] + mc8[7]),
                _ => 0.0,
            }
            
        };
		if mdb.iter().any(|x| *x > 0) {
			let ploidy = mdb.len() / ns;
			if ploidy == 0 { continue }
			let ctg_name = &curr_ctg.as_ref().unwrap().1;
			buffer.clear();
			write!(buffer, "{}\t{}\t{}", ctg_name, pos + 1, rs_id.unwrap())?;
			for gt in mdb.chunks(ploidy) {
				buffer.push(b'\t');
				for i in gt { buffer.push(get_gt(*i)) }	
                if ploidy == 2 {
                    match (gt[0], gt[1]) {
                        (0, 0) => write!(&mut buffer, "\t0.000")?,
                        (c1, c2) if c1 == c2 => write!(&mut buffer, "\t1.000")?,
                        (0, c2) => write!(&mut buffer, "\t{:.3}", get_baf(0, c2))?,
                        (c1, 0) => write!(&mut buffer, "\t{:.3}", get_baf(0, c1))?,
                        (c1, c2) => write!(&mut buffer, "\t{:.3}", get_baf(c1, c2))?,
                    }    
                } else { write!(&mut buffer, "\t.")? }
			}
			buffer.push(b'\n');
			out.write_all(&buffer)?;
		}
	}
	drop(out);	
	for x in procs.iter() { 
        if x.s.send(true).is_err() { warn!("Couldn't send closing message") }
    }
	for x in procs.drain(..) { x.th.join().unwrap() }
	Ok(())
}

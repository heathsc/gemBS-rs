use std::io;
use std::thread;
use std::sync::{Arc, atomic::{Ordering, AtomicUsize}};
use std::collections::HashMap;
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Select};

use crate::config::*;
use super::snp::{*, read_bed::snp_from_bed, SnpBlock};
use utils::compress::get_reader;

struct ReaderBuf {
	buffer: HashMap<Arc<str>, Vec<RawSnp>>,	
	limit: usize,
}

impl ReaderBuf {
	fn new(limit: usize) -> Self {
		Self{buffer: HashMap::new(), limit}	
	}
	fn add_snp(&mut self, snp: Snp) {
		let (raw_snp, contig) = snp.components();
		let cname = contig.ref_name();
		let v = self.buffer.entry(cname).or_insert(Vec::new());	
		v.push(raw_snp);
		if v.len() >= self.limit {
			let v = self.buffer.remove(contig.name()).unwrap();
			let sb = SnpBlock::new(contig.clone(), v);
			contig.send_message(sb);
		}
	}	
}

fn read_bed_file(conf: &Config, file: Option<&str>, rbuf: &mut ReaderBuf) -> io::Result<()> {
	let mut builder = SnpBuilder::new(conf.ctg_hash(), conf.pref_hash());
	let mut rdr = get_reader(file)?;
	info!("Reading from {:?}", file);
	let mut buf = String::new();
	loop {
		match rdr.read_line(&mut buf) {
			Ok(0) => break,
			Ok(_) => {
				if buf.starts_with("track") {
					
				} else {
					if let Some(snp) = snp_from_bed(&buf, &mut builder) { rbuf.add_snp(snp) }
				}
				buf.clear();
			},
			Err(e) => return Err(e),
		}
	}
	info!("Finished reading from {:?}", file);
	Ok(())
}

fn read_bed_thread(conf: Arc<Config>, ifiles: Arc<InputFiles>, mut rbuf: ReaderBuf) {
	while let Some(f) = ifiles.next_file() {
		let _ = read_bed_file(conf.as_ref(), Some(f), &mut rbuf);
	}
}

fn store_snp_block(sb: &SnpBlock) {
	
}

fn store_thread(conf: Arc<Config>, control_receiver: Receiver<bool>, thread_id: usize) {
	let mut ending = false;

	loop {	
		// Build up list of channels to watch
		let ctgs = conf.ctg_hash().get_avail_contig_list();
		let mut sel = Select::new();
		for(_, r) in ctgs.iter() { sel.recv(&r); }
		if !ending {
			let ctr_idx = sel.recv(&control_receiver);
			if let Ok(op) = sel.ready_timeout(Duration::from_millis(100)) {
				match op {
					idx if idx == ctr_idx => match control_receiver.recv() {
						Ok(_) => {
							debug!("Store thread {} received shutdown signal", thread_id);
							ending = true;
						},		
						Err(e) => panic!("Store thread {} - Error receiving message from control channel: {}", thread_id, e),
					},
					idx => {
						// Try to bind this contig
						if let Some(g) = ctgs[idx].0.try_bind() { 
							for sb in g.recv().try_iter() {
								store_snp_block(&sb);
							}
						}				
					},
				}	
			}			
		} else {
			let mut processed = false;
			if !ctgs.is_empty() {
				while let Ok(idx) = sel.try_ready() {
					// Try to bind this contig
					if let Some(g) = ctgs[idx].0.try_bind() { 
						for sb in g.recv().try_iter() {
							store_snp_block(&sb);
							processed = true;
						}
					}
				}
			}
			if !processed { break }	
		}
	}
	debug!("Store thread {} finishing up", thread_id);
	
}

struct InputFiles {
	idx: AtomicUsize,
	files: Box<[String]>,
}

impl InputFiles {
	fn next_file(&self) -> Option<&str> {
		let idx = self.idx.fetch_add(1, Ordering::AcqRel);
		if idx >= self.files.len() { None }
		else { Some(&self.files[idx]) }
	}
}

pub fn process(conf: Config, files: Box<[String]>) -> io::Result<()> {
	if files.is_empty() {
//		let mut rdr = ReaderBuf::new(128);
//		read_bed_file(&conf, None, &mut rdr)?;
	} else {
		let conf_ref = Arc::new(conf);
		let n_readers = conf_ref.threads().min(files.len());
		let mut readers = Vec::with_capacity(n_readers);
		let ifiles = Arc::new(InputFiles{idx: AtomicUsize::new(0), files});
		for _ in 0..n_readers {
			let cf = conf_ref.clone();
			let inp_files = ifiles.clone();			
			let rdr = ReaderBuf::new(256);
			let th = thread::spawn(move || {read_bed_thread(cf, inp_files, rdr)});
			readers.push(th);
		}
		let n_storers = conf_ref.threads();
		let mut storers = Vec::with_capacity(n_storers);
		for ix in 0..n_storers {
			let (s, r) = bounded(1);
			let cref = conf_ref.clone();
			let th = thread::spawn(move || {store_thread(cref, r, ix)});
			storers.push((th, s));
		}		
		for th in readers { th.join().unwrap(); }
		for (_, s) in storers.iter() { s.send(true).unwrap() }
		for (th, _) in storers { th.join().unwrap(); }
	}

	Ok(())	
}
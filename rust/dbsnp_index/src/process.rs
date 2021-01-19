use std::io;
use std::thread;
use std::sync::{Arc, atomic::{Ordering, AtomicUsize}};

use crossbeam_channel::bounded;

use crate::config::*;
use super::snp::*;
use super::contig::Contig;
use super::read::{ReaderBuf, read_thread, proc_read_thread};
use super::write::write_thread;
use super::compress::compress_thread;

pub struct AtomicServer<T> {
	idx: AtomicUsize,
	list: Box<[T]>,
}

impl <T>AtomicServer<T> {
	pub fn new(list: Box<[T]>) -> Self { Self{idx: AtomicUsize::new(0), list} }
	pub fn next_item(&self) -> Option<&T> {
		let idx = self.idx.fetch_add(1, Ordering::AcqRel);
		if idx >= self.list.len() { None }
		else { Some(&self.list[idx]) }
	}
}

pub fn process(conf: Config, files: Box<[DbInput]>) -> io::Result<()> {
	let conf_ref = Arc::new(conf);
	let n_readers = conf_ref.jobs().min(files.len());
	let n_proc_threads = conf_ref.threads();
	let mut readers = Vec::with_capacity(n_readers);
	let ifiles = Arc::new(AtomicServer::new(files));
	let (global_send, global_recv) = bounded(n_proc_threads * 32);
	for _ in 0..n_readers {
		let cf = conf_ref.clone();
		let inp_files = ifiles.clone();		
		let s = global_send.clone();
		let th = thread::spawn(move || {read_thread(cf, inp_files, s)});
		readers.push(th);
	}
	let mut proc_threads = Vec::with_capacity(n_proc_threads);
	for _ in 0..n_proc_threads {
		let cf = conf_ref.clone();
		let r = global_recv.clone();
		let th = thread::spawn(move || {proc_read_thread(cf, r)});
		proc_threads.push(th);
	}	
	let n_storers = conf_ref.jobs();
	let mut storers = Vec::with_capacity(n_storers);
	for ix in 0..n_storers {
		let (s, r) = bounded(1);
		let cref = conf_ref.clone();
		let th = thread::spawn(move || {store_thread(cref, r, ix)});
		storers.push((th, s));
	}		
	for th in readers { th.join().unwrap(); }
	drop(global_send);
	for th in proc_threads { th.join().unwrap(); }
	for (_, s) in storers.iter() { s.send(true).unwrap() }
	for (th, _) in storers { th.join().unwrap(); }
	let ctg_stats_vec = conf_ref.ctg_hash().get_ctg_stats();
	let ctgs: Vec<Arc<Contig>> = ctg_stats_vec.iter().map(|x| x.0.clone()).collect();
	let ctg_list = Arc::new(AtomicServer::new(ctgs.into_boxed_slice()));
	let n_compressors = conf_ref.threads();
	let mut compressors = Vec::with_capacity(n_compressors);
	let (s, r) = bounded(n_compressors);
	for ix in 0..n_compressors {
		let sc = s.clone();
		let ctgs = ctg_list.clone();
		compressors.push(thread::spawn(move || {compress_thread(ctgs, sc, ix)}));
	}
	let writer = thread::spawn(move || write_thread(conf_ref, r));
	for th in compressors {	th.join().unwrap()}
	drop(s);
	writer.join().unwrap();
	for (ctg, cstats) in ctg_stats_vec.iter() {
		println!("Contig {}: n_snps {}, n_selected_snps {}", ctg.name(), cstats.n_snps(), cstats.n_selected_snps());
	}
	Ok(())	
}
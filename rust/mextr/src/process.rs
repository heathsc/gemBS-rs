use std::io;
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{bounded, unbounded, Receiver};

use r_htslib::{VcfHeader, BcfSrs, htsThreadPool};
use crate::config::ConfHash;
use super::read_vcf::read_vcf;
use super::read_vcf::unpack::RecordBlock;
use super::output::*;
use super::output::md5::md5_thread;
use super::output::tabix::tabix_thread;

pub type Recv = Receiver<(usize, Arc<RecordBlock>)>;
pub type TPool = Arc<Option<htsThreadPool>>;

type ExtraFn = fn(Arc<ConfHash>, Receiver<bool>);
type OutputFn = fn(Arc<ConfHash>, Arc<VcfHeader>, Recv, TPool);

const OUTPUTS: [(&str, OutputFn); 3] = [
	("cpgfile", output_cpg_thread),
	("noncpgfile", output_noncpg_thread),
	("bed_methyl", output_bed_methyl_thread),
];

const EXTRAS: [(&str, ExtraFn); 2] = [("md5", md5_thread), ("tabix", tabix_thread)];

pub fn process(chash: ConfHash, mut sr: BcfSrs) -> io::Result<()> {
	let hdr = Arc::new(sr.get_reader_hdr(0)?.dup());
	let chash = Arc::new(chash);
	let mut out_threads = Vec::new();
	let mut out_channels = Vec::new();
	
	// Set up thread pool for htsFiles (both reading and writing)
	let nt = chash.get_int("threads");
	let mut thread_pool = if nt > 0 { htsThreadPool::init(nt.min(4)) } else { None };
	if let Some(tp) = thread_pool.as_mut() { sr.get_reader(0)?.file().set_thread_pool(tp); }
	
	// We'll be sharing the pool with the output threads, so wrap it in an Arc.
	let thread_pool = Arc::new(thread_pool);
	
	// Set up output threads
	for (_, f) in OUTPUTS.iter().filter(|(s, _)| chash.get_str(s).is_some()) {
		let ch = chash.clone();
		let hd = hdr.clone();
		let tp = thread_pool.clone();
		let (s, r) = bounded(32);
		let th = thread::spawn(move || f(ch, hd, r, tp));
		out_threads.push(th);
		out_channels.push(s);
	}	
	if !out_threads.is_empty() {

		// Set up Md5 and tabix threads
		let mut extra_vec: Vec<_> = EXTRAS.iter().filter(|(s, _)| chash.get_bool(s)).map(|(_, f)| {
			let ch = chash.clone();
			let (s, r) = unbounded();
			let th = thread::spawn(move || f(ch, r));
			(th, s)							
		}).collect();

		read_vcf(chash, sr, hdr, 3, out_channels)?;
		for th in out_threads.drain(..) {
			th.join().unwrap();
		}
		
		for (th, s) in extra_vec.drain(..) {
			s.send(true).expect("Error sending message to extras thread");
			th.join().unwrap();
		}
	} else { error!("No output option seleted") }
	Ok(())
}

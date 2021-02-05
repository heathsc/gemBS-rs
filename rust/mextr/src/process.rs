use std::io;
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{bounded, Receiver};

use r_htslib::{VcfHeader, BcfSrs, htsThreadPool};
use crate::config::ConfHash;
use super::read_vcf::read_vcf;
use super::read_vcf::unpack::RecordBlock;

use super::output::*;

pub type Recv = Receiver<(usize, Arc<RecordBlock>)>;
pub type TPool = Arc<Option<htsThreadPool>>;
type OutputFn = fn(Arc<ConfHash>, Arc<VcfHeader>, Recv, TPool);

const OUTPUTS: [(&str, OutputFn); 3] = [
	("cpgfile", output_cpg_thread),
	("noncpgfile", output_noncpg_thread),
	("bed_methyl", output_bed_methyl_thread),
];

pub fn process(chash: ConfHash, mut sr: BcfSrs) -> io::Result<()> {
	let hdr = Arc::new(sr.get_reader_hdr(0)?.dup());
	let chash = Arc::new(chash);
	let mut out_threads = Vec::new();
	let mut out_channels = Vec::new();
	let nt = chash.get_int("threads");
	let compress = chash.get_bool("compress");
	let thread_pool = Arc::new(if nt > 0 && compress { htsThreadPool::init(nt) } else { None });
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
		read_vcf(chash, sr, hdr, 3, out_channels)?;
		for th in out_threads.drain(..) {
			th.join().unwrap();
		}
	} else { error!("No output option seleted") }
	Ok(())
}
use std::sync::Arc;
use std::io::Write;

use crossbeam_channel::Sender;
use zstd::block::compress;

use super::process::AtomicServer;
use super::contig::Contig;
use super::write::{write_u128, write_u32, write_u16};

pub struct CompressBlock {
	first_bin: u32,
	cbuf: Vec<u8>,	
}

impl CompressBlock {
	pub fn first_bin(&self) -> u32 { self.first_bin }
	pub fn cbuf(&self) -> &[u8] { &self.cbuf }
}

fn write_bin_inc<W: Write>(mut file: W, x: usize) {
	if x < 64 { file.write_all(&[x as u8]).expect("Write error"); }
	else if x < 256 { file.write_all(&[64, x as u8]).expect("Write error"); }
	else if x < 65536 { 
		file.write_all(&[128]).expect("Write error");
		write_u16(file, &[x as u16]).expect("Write error"); 
	} else {
		file.write_all(&[192]).expect("Write error");
		write_u32(file, &[x as u32]).expect("Write error"); 
	}
}

const BUF_SIZE: usize = 1048576 * 32;

fn compress_contig(ctg: &Contig) -> (Vec<CompressBlock>, usize) {
	let mut ubuf: Vec<u8> = Vec::with_capacity(BUF_SIZE + 4096);
	let mut max_size = 0;
	let mut cblocks = Vec::new();
	let mut cdata = ctg.data().write().unwrap();
	let mut first_bin = cdata.min_bin().unwrap();
	let mut total_usize = 0;
	let mut total_csize = 0;
	if let Some(bins) = cdata.bins() {
		let mut bin_inc = 0;
		let mut curr_bin = first_bin;
		let mut first = true;
		for mut b in bins.drain(..) {
			if let Some(mut bin) = b.take() {
				if first {
					first_bin += bin_inc;
					first = false;
				} else { write_bin_inc(&mut ubuf, bin_inc) }
				curr_bin += bin_inc + 1;
				bin_inc = 0;
				// Write mask
				write_u128(&mut ubuf, bin.mask()).expect("Write error");
				// Write names
				bin.write_names(&mut ubuf);
				if ubuf.len() >= BUF_SIZE {
					let cbuf = compress(&ubuf, 0).expect("Error when compressing");
					total_usize += ubuf.len();
					total_csize += cbuf.len();
					max_size = max_size.max(ubuf.len());
					ubuf.clear();
					cblocks.push(CompressBlock{first_bin: first_bin as u32, cbuf});
					first_bin = curr_bin;
					first = true;
				}
			} else { bin_inc += 1 }	
		}
		if !ubuf.is_empty() {
			let cbuf = compress(&ubuf, 0).expect("Error when compressing");
			total_usize += ubuf.len();
			total_csize += cbuf.len();
			max_size = max_size.max(ubuf.len());
			ubuf.clear();
			cblocks.push(CompressBlock{first_bin: first_bin as u32, cbuf});			
		}
	}	
	let z = if total_usize > 0 { (total_csize as f64) / (total_usize as f64) } else { 1.0 };
	debug!("contig {}, ubuf {}, cbuf {}, ratio: {}", ctg.name(), total_csize, total_usize, z);
	(cblocks, max_size)
}

pub fn compress_thread(ctgs: Arc<AtomicServer<Arc<Contig>>>, send: Sender<(Arc<Contig>, Vec<CompressBlock>, usize)>, thread_id: usize) {
	while let Some(ctg) = ctgs.next_item() {
		let mut max_size = 0;
		debug!("Compress thread {}: Compressing contig {}", thread_id, ctg.name());
		let (cblocks, s) = compress_contig(ctg);
		max_size = max_size.max(s);
		send.send((ctg.clone(), cblocks, max_size)).unwrap();
	}	
	debug!("Compress thread {} terminating", thread_id);
}

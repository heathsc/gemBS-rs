use std::io;
use std::sync::Arc;
use std::thread;
use std::cell::RefCell;
use crossbeam_channel::{unbounded, Sender};

use r_htslib::*;
use crate::config::*;

pub mod model;
pub mod unpack;
use unpack::*;

pub const BREC_BLOCK_SIZE: usize = 1024;

pub struct BrecBlock {
	idx: usize,
	buf: Vec<BcfRec>,
}

impl BrecBlock {
	fn new(sz: usize) -> io::Result<Self> {
		let mut buf = Vec::with_capacity(sz);
		for _ in 0..sz { buf.push(BcfRec::new()?)}
		Ok(Self{idx: 0, buf})
	}
	pub fn idx(&self) -> usize { self.idx} 
	pub fn buf(&mut self) -> &mut [BcfRec] { &mut self.buf }	
}

pub fn read_vcf(chash: Arc<ConfHash>, mut sr: BcfSrs, hdr: Arc<VcfHeader>, n_slaves: usize, channel_vec: Vec<Sender<(usize, Arc<RecordBlock>)>>) -> io::Result<()> {
	let ns = hdr.nsamples();
	assert!(ns > 0);	
	if n_slaves > 1 {
		let (empty_s, empty_r) = unbounded();
		let (full_s, full_r) = unbounded();
		// Init blocks and add to empty queue
		for _ in 0..n_slaves * 2 {
			empty_s.send(BrecBlock::new(BREC_BLOCK_SIZE)?).map_err(|e| new_err(format!("Error adding to empty queue: {}",e)))?
		}
		// Create slave unpackers
		let mut slaves = Vec::with_capacity(n_slaves);
		let channel_vec = Arc::new(channel_vec);
		for _ in 0 .. n_slaves {
			let hd = hdr.clone();
			let ch = chash.clone();
			let cv = channel_vec.clone();
			let es = empty_s.clone();
			let fr = full_r.clone();
			let th = thread::spawn(move || unpack_vcf_slave(ch, hd, cv, es, fr));
			slaves.push(th);
		}
		// Read input file
		let mut idx = 0;
		let mut curr_blk = None;
		while sr.next_line() > 0 {
			// Get empty block if required
			if curr_blk.is_none() {
				if let Ok(mut blk) = empty_r.recv() {
					unsafe { blk.buf.set_len(0) }
					assert!(blk.buf.capacity() > 0);
					blk.idx = idx;
					idx += 1;
					curr_blk = Some(blk);
				} else {
					error!("Error from empty channel");
					break;
				}
			}
			let cb = curr_blk.as_mut().unwrap();
			// All elements are initialized so we can do this
			let i = cb.buf.len();
			unsafe { cb.buf.set_len(i + 1) }
			sr.swap_line(0, &mut cb.buf[i])?;
			if i + 1 == cb.buf.capacity() {
				full_s.send(curr_blk.take().unwrap()).map_err(|e| new_err(format!("Error ending to full queue: {}",e)))?;
			}
		}
		if let Some(cb) = curr_blk.take() { full_s.send(cb).map_err(|e| new_err(format!("Error ending to full queue: {}",e)))?}
		drop(full_s);
		for th in slaves.drain(..) { th.join().unwrap() }
		
	} else {	
		let mut udata = UnpackData::new(&chash, ns);
		let mut brec = BcfRec::new()?;
		while sr.next_line() > 0 {
			sr.swap_line(0, &mut brec)?;
			unpack_vcf(&mut brec, 0, &hdr, &mut udata, &channel_vec);	
		}
		// Send last block
		send_blk(&mut udata, &channel_vec);
	}
	Ok(())
}

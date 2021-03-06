use std::sync::Arc;
use std::collections::HashMap;

use crossbeam_channel::{Sender, Receiver};
use libc::{c_int, c_ulong};

use crate::config::ConfHash;
use crate::bbi::*;

const Z_OK:c_int = 0;

#[link(name = "z")]
extern "C" {
	fn compress(dest: *mut u8, destLen: *mut c_ulong, source: *const u8, sourceLen: c_ulong) -> c_int;
	fn compressBound(sourceLen: c_ulong) -> c_ulong;
}

fn compress_buf(inbuf: &[u8]) -> Vec<u8> {
	let mut cbound = unsafe{compressBound(inbuf.len() as c_ulong)};
	let mut out = Vec::with_capacity(cbound as usize);
	unsafe {
		if compress(out.as_mut_ptr(), &mut cbound, inbuf.as_ptr(),  inbuf.len() as c_ulong) != Z_OK { panic!("Error compressing buffer")}
		out.set_len(cbound as usize);
	}
	out
}

fn update_max_size(hash: &mut HashMap<BbiBlockType, usize>, bbi_type: BbiBlockType, sz: usize) {
	let curr_val = hash.entry(bbi_type).or_insert(0);
	*curr_val = (*curr_val).max(sz);
}

pub fn compress_bbi_thread(ch: Arc<ConfHash>, r: Receiver<BbiMsg>, ps: Sender<BbiMsg>) {
	info!("compress_bbi_thread starting up");
	let mut max_uncomp_size = HashMap::new();
	for msg in r.iter() {
		match msg {
			BbiMsg::Data((blk, v)) => {
				update_max_size(&mut max_uncomp_size, blk.bbi_type(), v.len());
				let mut cbuf = compress_buf(&v);
				cbuf.shrink_to_fit();
				ps.send(BbiMsg::Data((blk, cbuf))).expect("Error sending compressed data block");			
			},
			BbiMsg::ZData((blk, v, level)) => {
				update_max_size(&mut max_uncomp_size, blk.bbi_type(), v.len());
				let mut cbuf = compress_buf(&v);
				cbuf.shrink_to_fit();
				ps.send(BbiMsg::ZData((blk, cbuf, level))).expect("Error sending compressed data block");			
			},
			BbiMsg::EndOfSection(_) => ps.send(msg).expect("Error sending compressed data block"),
		}
	}
	for (bbi_type, sz) in max_uncomp_size.iter() {	ch.update_max_uncomp_size(*bbi_type, *sz) }
	info!("compress_bbi_thread shutting down");
}

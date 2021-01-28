use std::io::ErrorKind;
use std::str::{FromStr, from_utf8};
use std::collections::HashMap;
use std::fs::File;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use r_htslib::*;

///
/// We calculate the tabix index in parallel with the creation of the output file
/// to avoid having a (albeit) small wait after processing to make the index.
/// 
/// Since we need the compressed output to do this, we read from the generated
/// output file as it is being made.  The difficulty with doing this is handling
/// partial lines due to EOF being reached.  We deal with this by storing the file
/// offset in the BGZF file after the last successful read.  If we have a partial 
/// read, we seek to the previous offset and then wait for 050ms before retrying.
/// 
/// We could not use the bgzf_getline() function from htslib as this does not allow
/// a partial line to be distinguished, so a rust version was written in r_htslib.
/// 
pub fn tabix_thread(name: String, r: Receiver<bool>) {
	// First we wait until something has been written otherwise hts_open will not work properly
	let f = File::open(&name).expect("tabix_thread: file not found");
	let d = Duration::from_millis(250);
    loop {
		if f.metadata().expect("tabix_thread: couldn't access file").len() > 0 { break }
        match r.recv_timeout(d) {
			Ok(_) => return,
			Err(RecvTimeoutError::Timeout) => (),
			Err(e) => {
				error!("tabix_thread received error: {}", e);
				return;
			},
		}
    }
	// Avoid getting complaints about no EOF marker
	unsafe{hts_set_log_level(htsLogLevel::HTS_LOG_ERROR)}
	
	let mut f = HtsFile::new(&name, "rz").expect("tabix_thread: file not found");
	let mut bgzf = f.bgzf().expect("tabix_thread: file not bgzip compressed");
	info!("Starting tabix index generation");
	
	let mut line = Vec::new();
	let mut last_off: u64 = 0;
	let meta_char = b'#';
	let mut end = false;
	let mut error = false;
	let d = Duration::from_millis(500);
	let mut idx = None;
	let mut ctgs = HashMap::new();
	let mut prev_ctg: Vec<u8> = Vec::new();
	let mut prev_working_pos = 0;
	let mut last_x = 0;
    loop {
		loop {
			match bgzf.getline(b'\n', &mut line) {
				Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
					bgzf.clear_eof();
					if bgzf.seek(prev_working_pos) != 0 { 
						error!("tabix thread: Seek error");
						error = true;
					}
					break;
				},
				Err(e) => {
					error = true;
					error!("tabix_thread: error from getline(): {}", e);
					break;
				}
				Ok(_) => {
					prev_working_pos = bgzf.tell();
					if line.is_empty() || line[0] == meta_char {
						last_off = bgzf.tell() as u64;
						continue;
					} 
					if idx.is_none() { idx = Some(HtsIndex::init(0, HTS_FMT_TBI, last_off, 14, 5).expect("tabix thread - could not init index")); }
					// Parse line
					let seq_pos = {
						let mut seq = None;
						let mut pos = None;
						for (ix, s) in line.split(|c| *c == b'\t').take(2).enumerate() {
							if ix == 0 { seq = Some(s) }
							else { pos = <usize>::from_str(from_utf8(s).expect("Non UTF8 characters found")).ok() }
							// positions must be > 0
							if let Some(0) = pos { pos = None }
						}
						if let (Some(s), Some(p)) = (seq, pos) { Some((s, p)) } else { None }
					};
					if let Some((seq, pos)) = seq_pos {
						if seq != &prev_ctg[..] {
							prev_ctg = seq.to_vec();
							if ctgs.insert(prev_ctg.clone(), ctgs.len()).is_some() { panic!("tabix_index: File is not sorted") }
							debug!("Tabix thread: Added ctg {} with id {}", from_utf8(&prev_ctg).unwrap(), ctgs.get(&prev_ctg).unwrap());
							last_x = 0;
						}
						if pos <= last_x { panic!("tabix_index: File is not sorted") }
						last_x = pos;
						if idx.as_mut().unwrap().push(*ctgs.get(&prev_ctg).unwrap(), pos - 1, pos, bgzf.tell() as u64, true).is_err() {
							error!("Error returned from hts_idx_push()");
							error = true;
							break;
						}
					}
				},
			}
		}
		if end || error { break }	
        match r.recv_timeout(d) {
			Ok(_) => end = true,
			Err(RecvTimeoutError::Timeout) => (),
			Err(e) => {
				error!("tabix_thread received error: {}", e);
				break;
			},
		}
    }
	if !error {
		let mut idx = idx.unwrap_or_else(|| HtsIndex::init(0, HTS_FMT_TBI, last_off, 14, 5).expect("tabix thread - could not init index"));
		if idx.finish(bgzf.tell() as u64).is_err() { error!("Error returned from hts_idx_finish")}
		else {
			// Add metadata to index
			let mut x = [TBX_GENERIC, 1, 2, 0, b'#' as i32, 0, 0];
			let mut tmp = Vec::with_capacity(ctgs.len());
			let mut len = 0;
			for (cname, tid) in ctgs.drain() { 
				len += cname.len() + 1;
				tmp.push((cname, tid)) 
			}
			x[6] = len as i32;
			let mut meta = Vec::with_capacity(28 + len);
			for y in &x { for c in &y.to_le_bytes() { meta.push(*c) }}
			tmp.sort_by_key(|(_, tid)| *tid);
			for (cname, _) in tmp.drain(..) {
				for c in &cname { meta.push(*c) }
				meta.push(0);
			}
			assert_eq!(meta.len(), 28 + len);
			if idx.set_meta(&meta).is_err() { error!("Error retruned from hts_idx_set_meta") }
			else if idx.save_as(&name, HTS_FMT_TBI).is_err() { error!("Error retruned from hts_idx_save_as") }
		}
	}
	info!("Tabix index generation complete");
}

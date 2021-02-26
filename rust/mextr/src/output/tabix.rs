use std::io::ErrorKind;
use std::fs::metadata;
use std::str::{FromStr, from_utf8};
use crossbeam_channel::{Receiver, RecvTimeoutError};
use std::time::Duration;
use std::collections::HashMap;
use std::sync::Arc;
use crate::config::ConfHash;

use r_htslib::*;

struct TabixFile {
	name: String,
	file: Option<HtsFile>,
	idx: Option<HtsIndex>,
	prev_working_pos: HtsPos,
	prev_ctg: Vec<u8>,
	ctgs: HashMap<Vec<u8>, usize>, 
	last_off: u64,
	last_x: usize,
	line_no: usize,
	skip: usize,
	error: bool,
}

impl TabixFile {
	fn new<S: AsRef<str>>(name: S, skip: usize) -> Self {
		let name = name.as_ref().to_owned();
		
		Self {
			name,
			last_off: 0,
			prev_working_pos: 0,
			last_x: 0,
			line_no: 0,
			skip,
			idx: None,
			error: false,
			prev_ctg: Vec::new(),
			ctgs: HashMap::new(),
			file: None
		}
	}
	
	fn update(&mut self, line: &mut Vec<u8>) {
		if !self.error {
			if self.file.is_some() { self.process(line) } 
			else { self.open_file(line) }
		}
	}
	
	fn open_file(&mut self, line: &mut Vec<u8>) {
		if let Ok(m) = metadata(&self.name) {
			if m.len() > 0 {
				self.file = Some(HtsFile::new(&self.name, "rz").unwrap_or_else(|e| panic!("tabix_thread: could not open file {}: {}", &self.name, e)));
				self.process(line);
			}
		}
	}
	
	fn process(&mut self, line: &mut Vec<u8>) {
		let bgzf = self.file.as_mut().unwrap().bgzf().expect("tabix_thread: file not bgzip compressed");
		let meta_char = b'#';
		self.error = loop {
			match bgzf.getline(b'\n', line) {
				Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
					bgzf.clear_eof();
					break if bgzf.seek(self.prev_working_pos) != 0 { 
						error!("tabix thread: Seek error");
						true
					} else { false }
				},
				Err(e) => {
					error!("tabix_thread: error from getline(): {}", e);
					break true;
				}
				Ok(_) => {
					self.prev_working_pos = bgzf.tell();
					self.line_no += 1;
					// Skip initial lines if required (+ lines starting with #) 
					if line.is_empty() || line[0] == meta_char || self.line_no <= self.skip {
						self.last_off = bgzf.tell() as u64;
						continue;
					} 
					if self.idx.is_none() { self.idx = Some(HtsIndex::init(0, HTS_FMT_TBI, self.last_off, 14, 5).expect("tabix thread - could not init index")); }
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
						if seq != &self.prev_ctg[..] {
							self.prev_ctg = seq.to_vec();
							if self.ctgs.insert(self.prev_ctg.clone(), self.ctgs.len()).is_some() { panic!("tabix_index: File {} is not sorted", &self.name) }
							debug!("Tabix thread: Added ctg {} with id {} for file {}", from_utf8(&self.prev_ctg).unwrap(), self.ctgs.get(&self.prev_ctg).unwrap(), &self.name);
							self.last_x = 0;
						}
						if pos <= self.last_x { panic!("tabix_index: File is not sorted") }
						self.last_x = pos;
						if self.idx.as_mut().unwrap().push(*self.ctgs.get(&self.prev_ctg).unwrap(), pos - 1, pos, bgzf.tell() as u64, true).is_err() {
							error!("Error returned from hts_idx_push()");
							break true;
						}
					}
				},
			}
		};
	}
	
	fn finish(&mut self) {
		if !self.error {
			let bgzf = self.file.as_mut().unwrap().bgzf().expect("tabix_thread: file not bgzip compressed");
			if self.idx.is_none() { self.idx = Some(HtsIndex::init(0, HTS_FMT_TBI, self.last_off, 14, 5).expect("tabix thread - could not init index")); }
			let idx = self.idx.as_mut().unwrap();
			if idx.finish(bgzf.tell() as u64).is_err() { error!("Error returned from hts_idx_finish")}
			else {
				// Add metadata to index
				let mut x = [TBX_GENERIC, 1, 2, 0, b'#' as i32, 0, 0];
				let mut tmp = Vec::with_capacity(self.ctgs.len());
				let mut len = 0;
				for (cname, tid) in self.ctgs.drain() { 
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
				else if idx.save_as(&self.name, HTS_FMT_TBI).is_err() { error!("Error retruned from hts_idx_save_as") }
			}
		}
	}
}

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
pub fn tabix_thread(chash: Arc<ConfHash>, r: Receiver<bool>) {
	debug!("tabix_thread starting up");
	// Avoid getting complaints about no EOF marker
	unsafe{hts_set_log_level(htsLogLevel::HTS_LOG_ERROR)}
	
	let d = Duration::from_millis(500);
	let mut line = Vec::new();
	let mut tabix_files = Vec::new();
	let mut end = false;
	let mut n_files = 0;
	let mut new_files = chash.n_out_files() > n_files;
	loop {
		if new_files {
			for (s, _) in chash.out_files().drain(tabix_files.len()..).filter(|(_, fg)| *fg) {
				debug!("tabix_thread: Adding file {}", s);
				let skip_lines = if chash.get_bool("no_header") { 0 } else { 1 };
				tabix_files.push(TabixFile::new(s, skip_lines))
			}
			n_files = chash.n_out_files();
		}
		new_files = chash.n_out_files() > n_files;
		for m in tabix_files.iter_mut() { m.update(&mut line) }
		if new_files { continue }
		if end { break }
        match r.recv_timeout(d) {
			Ok(_) => end = true,
			Err(RecvTimeoutError::Timeout) => (),
			Err(e) => {
				error!("tabix_thread received error: {}", e);
				break;
			},
		}		
	}	
	for mut m in tabix_files.drain(..) { m.finish() } 
	debug!("tabix_thread shutting down");
}



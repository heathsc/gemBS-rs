use std::io::{Write, Read};
use std::fs::File;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use std::time::Duration;
use md5::{Md5, Digest};
use std::sync::Arc;
use crate::config::ConfHash;

///
/// We calculate the md5 index in parallel with the creation of the output file
/// to avoid having a (albeit) small wait after processing to make the index.
/// 
/// Since we need the compressed output to do this, we read from the generated
/// output file as it is being made.  If we get to the EOF, we wait for 500ms and
/// try again, until we get a signal that the file has been completed.
/// 

struct Md5File {
	name: String,
	file: File,
	md5: Md5,	
}

impl Md5File {
	fn new<S: AsRef<str>>(name: S) -> Self {
		let name = name.as_ref().to_owned();
		let file = File::open(&name).unwrap_or_else(|e| panic!("md5_thread: could not open file {}: {}", &name, e));
		let md5 = Md5::new();
		Self{name, file, md5}
	}

	fn update(&mut self, buf: &mut [u8]) {
		loop {
			let n = self.file.read(&mut buf[..]).unwrap_or_else(|e| panic!("md5_thread: error reading from file {}: {}", self.name, e));
			if n == 0 { break } 
			self.md5.update(&buf[..n]);
		}			
	}

	fn finalize(self) {
		let mut tbuf: Vec<u8> = Vec::with_capacity(32);
		for i in self.md5.finalize() { write!(&mut tbuf, "{:02x}", i).unwrap(); }	
		let md5sig = std::str::from_utf8(&tbuf).unwrap();
		let name_md5 = format!("{}.md5", self.name);
		let mut of = File::create(&name_md5).expect("Couldn't create md5 output");
		writeln!(of, "{}  {}", md5sig, &self.name).unwrap();
	}	
}

pub fn md5_thread(chash: Arc<ConfHash>, r: Receiver<bool>) {

	debug!("md5_thread starting up");
	let d = Duration::from_millis(500);
	let mut buf = [0; 8192];
	let mut md5_files = Vec::new();
	let mut new_files = chash.n_out_files() > md5_files.len();
	loop {
		if new_files {
			for s in chash.out_files().drain(md5_files.len()..) {
				debug!("md5_thread: Adding file {}", s);
				md5_files.push(Md5File::new(s))
			}
		}
		new_files = chash.n_out_files() > md5_files.len();
		for m in md5_files.iter_mut() { m.update(&mut buf) }
		if new_files { continue }
        match r.recv_timeout(d) {
			Ok(_) => break,
			Err(RecvTimeoutError::Timeout) => (),
			Err(e) => {
				error!("md5_thread received error: {}", e);
				break;
			},
		}		
	}	
	for mut m in md5_files.drain(..) { 
		m.update(&mut buf);
		m.finalize();
	}
	debug!("md5_thread shutting down");
}

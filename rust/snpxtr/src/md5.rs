use std::io::{Write, Read};
use std::fs::File;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;
use md5::{Md5, Digest};

///
/// We calculate the md5 index in parallel with the creation of the output file
/// to avoid having a (albeit) small wait after processing to make the index.
/// 
/// Since we need the compressed output to do this, we read from the generated
/// output file as it is being made.  If we get to the EOF, we wait for 500ms and
/// try again, until we get a signal that the file has been completed.
/// 

pub fn md5_thread(name: String, r: Receiver<bool>) {
	let mut f = File::open(&name).expect("md5_thread: file not found");
	
	let mut buf = [0; 8192];
	let mut md5 = Md5::new();
	let d = Duration::from_millis(500);
    loop {
		loop {
			let n = f.read(&mut buf[..]).expect("md5_thread - error reading");
			if n == 0 { break } 
			md5.update(&buf[..n]);
		}	
        match r.recv_timeout(d) {
			Ok(_) => break,
			Err(RecvTimeoutError::Timeout) => (),
			Err(e) => {
				error!("md5_thread received error: {}", e);
				break;
			},
		}
    }
	loop {
		let n = f.read(&mut buf[..]).expect("md5_thread - error reading");
		if n == 0 { break } 
		md5.update(&buf[..n]);
	}	
	let mut tbuf: Vec<u8> = Vec::with_capacity(32);
	for i in md5.finalize() { write!(&mut tbuf, "{:02x}", i).unwrap(); }	
	let md5sig = std::str::from_utf8(&tbuf).unwrap();
	let name_md5 = format!("{}.md5", &name);
	let mut of = File::create(&name_md5).expect("Couldn't create md5 output");
	writeln!(of, "{}  {}", md5sig, &name).unwrap();
}

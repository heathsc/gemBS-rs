use std::str::FromStr;
use std::io;

use super::*;
use crate::snp::SnpBuilder;
use crate::config::Config;
use crate::process::AtomicServer;

use utils::compress::get_reader;

pub fn snp_from_bed(s: &str, rb: &mut SnpBuilder) -> Option<Snp> {
	let v: Vec<&str> = s.split('\t').collect();
	if v.len() > 4 {
		let x = <u32>::from_str(&v[1]).ok()?;
		let y = <u32>::from_str(&v[2]).ok()?;
		if y > x && y - x == 1 { return rb.mk_snp(v[3], v[0], y, None)}
	}
	None	
}

fn read_bed_file(conf: &Config, file: Option<&str>, rbuf: &mut ReaderBuf) -> io::Result<()> {
	let mut builder = SnpBuilder::new(conf.ctg_hash());
	let mut rdr = get_reader(file)?;
	info!("Reading from {}", file.unwrap_or("<stdin>"));
	let mut buf = String::new();
	loop {
		match rdr.read_line(&mut buf) {
			Ok(0) => break,
			Ok(_) => {
				if buf.starts_with("track") { conf.cond_set_description(&buf); }
				else if let Some(snp) = snp_from_bed(&buf, &mut builder) { rbuf.add_snp(snp) }
				buf.clear();
			},
			Err(e) => return Err(e),
		}
	}
	info!("Finished reading from {:?}", file);
	Ok(())
}

pub fn read_bed_thread(conf: Arc<Config>, ifiles: Arc<AtomicServer<String>>, mut rbuf: ReaderBuf) {
	while let Some(f) = ifiles.next_item().map(|s| s.as_str()) {
		let file = if f == "-" { None } else { Some(f) }; 
		let _ = read_bed_file(conf.as_ref(), file, &mut rbuf);
	}
}


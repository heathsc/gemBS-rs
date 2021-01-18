use std::sync::Arc;
use std::collections::HashMap;
use std::io;

use utils::compress::get_reader;

use super::snp::{RawSnp, Snp, SnpBlock};
use crate::config::{IType, Config};
use crate::process::AtomicServer;
use crate::snp::SnpBuilder;
use crate::contig::Contig;

pub mod read_bed;
pub mod read_json;
pub mod read_vcf;

use read_bed::process_bed_line;
use read_json::process_json_line;
use read_vcf::process_vcf_line;

pub struct ReaderBuf {
	buffer: HashMap<Arc<str>, (Vec<RawSnp>, Arc<Contig>)>,	
	limit: usize,
}

impl ReaderBuf {
	pub fn new(limit: usize) -> Self {
		Self{buffer: HashMap::new(), limit}	
	}
	pub fn add_snp(&mut self, snp: Snp) {
		let (raw_snp, contig) = snp.components();
		let cname = contig.ref_name();
		let (v, _) = self.buffer.entry(cname).or_insert((Vec::new(), contig.clone()));	
		v.push(raw_snp);
		if v.len() >= self.limit {
			let (v, _) = self.buffer.remove(contig.name()).unwrap();
			let sb = SnpBlock::new(contig.clone(), v);
			contig.send_message(sb);
		}
	}	
	pub fn flush(&mut self) {
		for (_, (v, contig)) in self.buffer.drain() {
			let sb = SnpBlock::new(contig.clone(), v);
			contig.send_message(sb);			
		}
	}
}

fn check_file_type(buf: &str) -> IType {
	if buf.starts_with('{') { IType::Json }
	else if buf.starts_with("##fileformat=VCF") { IType::Vcf }
	else { IType::Bed }
}

fn read_input_file(conf: &Config, file: Option<&str>, rbuf: &mut ReaderBuf) -> io::Result<()> {
	let mut builder = SnpBuilder::new(conf.ctg_hash());
	let mut rdr = get_reader(file)?;
	info!("Reading from {}", file.unwrap_or("<stdin>"));
	let mut itype = conf.input_type();
	let mut buf = String::new();
	loop {
		match rdr.read_line(&mut buf) {
			Ok(0) => break,
			Ok(_) => {
				if let IType::Auto = itype { itype = check_file_type(&buf) }
				match itype {
					IType::Bed => process_bed_line(conf, &buf, &mut builder, rbuf),
					IType::Json => process_json_line(&buf, &mut builder, rbuf),
					IType::Vcf => process_vcf_line(&buf, &mut builder, rbuf),
					_ => panic!("Unknown file type"),
				}
				buf.clear();
			},
			Err(e) => return Err(e),
		}
	}
	rbuf.flush();
	info!("Finished reading from {:?}", file);
	Ok(())
}

pub fn read_thread(conf: Arc<Config>, ifiles: Arc<AtomicServer<String>>, mut rbuf: ReaderBuf) {
	while let Some(f) = ifiles.next_item().map(|s| s.as_str()) {
		let file = if f == "-" { None } else { Some(f) }; 
		let _ = read_input_file(conf.as_ref(), file, &mut rbuf);
	}
}

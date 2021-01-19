use std::sync::Arc;
use std::collections::HashMap;
use std::io::{self, BufRead};

use crossbeam_channel::{Sender, Receiver};
use r_htslib::*;

use utils::compress::get_reader;

use super::snp::{RawSnp, Snp, SnpBlock};
use crate::config::{IType, Config, DbInput};
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

pub fn proc_read_thread(conf: Arc<Config>, r: Receiver<(Vec<String>, IType)>) {
	let mut rbuf = ReaderBuf::new(256);
	let mut builder = SnpBuilder::new(conf.ctg_hash());
	for (mut v, itype) in r.iter() {
		for buf in v.drain(..) {
			match itype {
				IType::Bed => process_bed_line(&conf, &buf, &mut builder, &mut rbuf),
				IType::Json => process_json_line(&buf, &mut builder, &mut rbuf),
				IType::Vcf => process_vcf_line(&buf, &mut builder, &mut rbuf),
				_ => panic!("Unknown file type"),
			}
		}
	}
	rbuf.flush();
}

fn read_input_file(conf: Arc<Config>, file: Option<&str>, send: &Sender<(Vec<String>, IType)>) -> io::Result<()> {
	let rdr = get_reader(file)?;
	info!("Reading from {}", file.unwrap_or("<stdin>"));
	let mut itype = conf.input_type();
	let size = 256;
	let mut buf_vec = Vec::with_capacity(size);
	for line in rdr.lines() {
		match line {
			Ok(buf) => {
				if let IType::Auto = itype { itype = check_file_type(&buf) }
				buf_vec.push(buf);
				if buf_vec.len() == size {					
					send.send((buf_vec, itype)).expect("Error sending message to read processing thread");
					buf_vec = Vec::with_capacity(size);
				}
			},
			Err(e) =>return Err(e),
		}
	}
	if !buf_vec.is_empty() { send.send((buf_vec, itype)).expect("Error sending message to read processing thread") }
	info!("Finished reading from {:?}", file);
	Ok(())
}

fn read_vcf_contig(file: &str, ctg: &str, tid: libc::c_int, send: &Sender<(Vec<String>, IType)>) -> io::Result<()> {
	let mut hfile = HtsFile::new(file, "r")?;
	let mut tbx = Tbx::new(file)?;
	let mut hitr = tbx.tbx_itr_queryi(tid, 0, HTS_POS_MAX)?;
	info!("Reading from {}:{}", file, ctg);
	let size = 256;
	let mut buf_vec = Vec::with_capacity(size);
	let mut kstr = kstring_t::new();
	loop {
		match hitr.tbx_itr_next(&mut hfile, &mut tbx, kstr) {
			TbxReadResult::Ok(s) => {
				buf_vec.push(s.to_str().expect("File not Utf8").to_owned());
				if buf_vec.len() == size {
					send.send((buf_vec, IType::Vcf)).expect("Error sending message to read processing thread");
					buf_vec = Vec::with_capacity(size);
				}
				kstr = s;
			},
			TbxReadResult::EOF => break,
			TbxReadResult::Error => {
				error!("Error reading from file {}:{}", file, ctg);
				break;
			},
		}
	}
	if !buf_vec.is_empty() { send.send((buf_vec, IType::Vcf)).expect("Error sending message to read processing thread") }
	info!("Finished reading from {}:{}", file, ctg);
	Ok(())
}

pub fn read_thread(conf: Arc<Config>, ifiles: Arc<AtomicServer<DbInput>>, send: Sender<(Vec<String>, IType)>) {
	while let Some(f) = ifiles.next_item() {
		match f {
			DbInput::File(fname) => {
				let file = if fname == "-" { None } else { Some(fname.as_str()) };
				let _ = read_input_file(conf.clone(), file, &send);
			},	 
			DbInput::VcfContig(file, ctg, tid) => { let _ = read_vcf_contig(file.as_str(), ctg.as_str(), *tid, &send); },
		}	
	}
}

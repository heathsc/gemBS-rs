use utils::compress;
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{fmt, env, fs};
use std::io::{BufRead, BufReader, Write, Error, ErrorKind};
use std::ffi::OsStr;
use std::str::FromStr;

use md5::{Md5, Digest};
use regex::Regex;
use lazy_static::lazy_static;

use crate::config::GemBS;
use crate::common::defs::Section;

fn get_env_var<K: AsRef<OsStr>>(key: K) -> Option<String> {
	if let Ok(x) = env::var(key) { 
		if x.is_empty() { None } else { Some(x) }
	} else { None }
}

// Follows the logic in htslib/cram/cram_io.c to find
// the path for the reference cache
fn get_cache_path() -> String {
	if let Some(x) = get_env_var("REF_CACHE") { x }
	else {
		let base = if let Some(x) = get_env_var("XDG_CACHE_HOME") { x }
		else if let Some(mut x) = get_env_var("HOME") { x.push_str("/.cache"); x }
		else {
			get_env_var("TMPDIR").unwrap_or_else(|| get_env_var("TMP").unwrap_or_else(|| String::from("/tmp")))
		};
		format!("{}/hts-ref/%2s/%2s/%s", base)
	} 
}

// For the MD5 calculation we skip everything with ascii value < 33 or > 126
// and all letters should be uppercase.  The quickest way to do this is with
// a lookup table
fn init_filter() -> [u8; 256] {
	let mut tab = [0; 256];
	const LO: u8 = 33;
	const HI: u8 = 127;
	for i in LO..HI { tab[i as usize] = i.to_ascii_uppercase(); }
	tab
}

struct Md5Digest {
	digest: [u8; 16]
}

impl Md5Digest {
	pub fn new() -> Self {
		Md5Digest{digest: [0; 16]}
	}
	pub fn set(&mut self, x: &[u8]) -> Result<(), String> {
		if x.len() == 16 {
			self.digest[..16].clone_from_slice(&x[..16]);
			Ok(())
		} else {
			Err(format!("Md5Digest::set requires [u8; 16].  Found [u8; {}]", x.len()))
		}
	}
}

impl fmt::Display for Md5Digest {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		for i in self.digest.iter() { write!(f, "{:02x}", i)?; }
		Ok(())
	}
}

struct Md5Contig {
	name: String,
	md5: Md5Digest,	
	len: usize,
}

impl fmt::Display for Md5Contig {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut iter = self.name.split_whitespace();		
		write!(f, "{}\tLN:{}\tM5:{}", iter.next().unwrap_or(""), self.len, self.md5)?;
		for s in iter {
			if s.starts_with("AS:") || s.starts_with("SP:") { write!(f, "\t{}", s)?; }
		}
		Ok(())
	}
}

impl Md5Contig {
	fn new<T: AsRef<str>>(name: T) -> Self {
		Md5Contig{name: name.as_ref().to_owned(), md5: Md5Digest::new(), len: 0}
	}
	fn handle_cache(&self, cache_path: &str, cache_buf: Vec<u8>) -> std::io::Result<Vec<u8>> {
		let fname = get_fname(cache_path, format!("{}", self.md5).as_str()).map_err(|e| Error::new(ErrorKind::Other, e))?;
		if !fname.exists() {
			// Make cache directories if required
			if let Some(d) = &fname.parent() { fs::create_dir_all(d)? }
			let mut wrt = compress::open_bufwriter(&fname)?;
			wrt.write_all(&cache_buf)?;
		}
		Ok(cache_buf)
	}
	fn check_cache<T: AsRef<Path>>(&self, gem_bs: &GemBS, path_str: &str, gref: T) -> Result<(), String> {
		let fname = get_fname(path_str, format!("{}", self.md5).as_str())?;
		if !fname.exists() {
			// Read in sequence information from gref
			let faidx_args = vec!("faidx", gref.as_ref().to_str().unwrap(), &self.name);
			let samtools_path = gem_bs.get_exec_path("samtools");
			info!("Creating cache for contig {} with md5 {}", self.name, self.md5);
			let mut rdr = BufReader::new(compress::open_read_filter(&samtools_path, &faidx_args).
				map_err(|e| format!("Couldn't get sequence data for contig {} from {}: {}", self.name, gref.as_ref().display(), e))?);
			let mut read_header = false;
			let mut line = String::with_capacity(128);
			let mut buf: Vec<u8> = Vec::with_capacity(16384);
			let filt_tab = init_filter();
			loop {
				gem_bs.check_signal()?;
				match rdr.read_line(&mut line) {
					Ok(0) => break,
					Ok(_) => {
						if let Some(ctg) = line.strip_prefix('>') {
							if read_header { return Err("Error - no FASTA header found".to_string()) }
							let name = ctg.trim_end();
							if name != self.name { return Err(format!("Expecting {}: read in {}", self.name, name)) }
							read_header = true;						
						} else if read_header {
							line.as_bytes().iter().filter(|x| filt_tab[**x as usize] != 0).for_each(|x| buf.push(*x));
						} else { return Err("Error: no FASTA sequence header line".to_string()) }
						line.clear();
					},
					Err(e) => return Err(format!("Error reading from file {}: {}", gref.as_ref().display(), e)),
				}
			}
			// Make cache directories if required
			if let Some(d) = &fname.parent() { fs::create_dir_all(d).map_err(|e| format!("Couldn't create cache directories for {}: {}", &fname.display(), e))?; }
			let mut wrt = compress::open_bufwriter(&fname).map_err(|e| format!("Couldn't open cache file {} for writing: {}", &fname.display(), e))?;
			wrt.write_all(&buf).map_err(|e| format!("Error writing to cache file {}: {}", &fname.display(), e))?;			
		}
		Ok(())	
	}
	fn process_ctg(&mut self, cache_path: &Option<String>, min_contig_size: &Option<usize>, md5_data: &mut Md5Data) -> std::io::Result<()> {
		let skip = if let Some(x) = min_contig_size { self.len < *x	} else { false };
		if !skip {
			writeln!(md5_data.output_md5, "{}", self)?;
			if let Some(path) = cache_path { md5_data.cache_buf = Some(self.handle_cache(path, md5_data.cache_buf.take().unwrap())?); }
			if let Some(buf) = &md5_data.cache_buf {
				writeln!(md5_data.output, ">{}", self.name)?;
				let l = buf.len();
				let mut i = 0;
				while i < l {
					let i1 = if l - i > 60 { i + 60 } else { l }; 
				 	md5_data.output.write_all(&buf[i..i1])?;
					writeln!(md5_data.output)?;
					i = i1;
				}
			}
		} 
		Ok(())	
	}
}


// Replace %s or %ds where d is an integer by sections of the md5 string to make a filename
fn get_fname(path_str: &str, md5: &str) -> Result<PathBuf, String> {
	lazy_static! { static ref RE: Regex = Regex::new(r"([^%]*)%(\d*)s([^%]*)").unwrap(); }
	let mut fname = String::new();
	let mut ix = 0;
	for c in RE.captures_iter(path_str) {
		fname.push_str(&c[1]);
		let n = if c[2].is_empty() {
			32 - ix	
		} else if let Ok(x) = <usize>::from_str(&c[2]) {
			if x + ix > 32 { 32 - ix } else { x }
		} else { 
			return Err(format!("Illegal format string for cache path: {}", path_str)) 
		};
		if n > 0 {
			let ix1 = ix + n;
			fname.push_str(&md5[ix..ix1]);
			ix = ix1;
		}
		fname.push_str(&c[3]);
	}
	Ok(PathBuf::from(&fname))
}

struct Md5Data {
	output: Box<dyn Write>,
	output_md5: Box<dyn Write>,
	cache_buf: Option<Vec<u8>>,
}

// Read reference fastas.  
// Write out bgzipped concatenated version to output_ref.  
// Calculate len and md5 for each contig and write out to ctg_md5. 
// If populate cache option set then save reference in cache using the 
// same naming logic as htslib
pub fn md5_fasta<P: AsRef<Path>, Q: AsRef<Path>, R: AsRef<Path>>(gem_bs: &GemBS, in_files: &[P], output_ref: Q, ctg_md5: R) -> Result<(), String> {

	let cache_path = if gem_bs.get_config_bool(Section::Index, "populate_cache") { 
		let cp = get_cache_path();
		debug!("reference cache_path = {}", cp);
		Some(cp)
	} else { None };
	let mut min_contig_size = gem_bs.get_config_int(Section::Index, "min_contig_size").map(|x| x as usize);
	let cache_buf: Option<Vec<u8>> = if cache_path.is_some() || min_contig_size.is_some() { Some(Vec::with_capacity(16384)) } else { None };
	let opath = output_ref.as_ref();
	let threads = gem_bs.get_threads(Section::Index);
	let bgzip_path = gem_bs.get_exec_path("bgzip");
	let filt_tab = init_filter();
	let output = compress::open_pipe_writer(opath, &bgzip_path, &["-@", format!("{}", threads).as_str()])
		.map_err(|e| format!("Couldn't open output {}: {}", opath.display(), e))?;
	let output_md5 = compress::open_bufwriter(ctg_md5.as_ref())
		.map_err(|e| format!("Couldn't open output {}: {}", ctg_md5.as_ref().display(), e))?;
	let mut md5_data = Md5Data{output, output_md5, cache_buf};
	
	debug!("Creating gemBS reference {}", opath.display());
	let mut current_ctg: Option<Md5Contig> = None;
	let mut line = String::with_capacity(128);
	let mut md5_buf: Vec<u8> = Vec::with_capacity(128);
	let mut md5_hasher = Md5::new();
	for p in in_files {
		let mut rdr = compress::open_bufreader(p).map_err(|e| format!("{}", e))?;
		debug!("Reading reference sequences from {}", p.as_ref().display());
		let now = Instant::now();
		loop {
			gem_bs.check_signal()?;
			match rdr.read_line(&mut line) {
				Ok(0) => break,
				Ok(_) => {
					if let Some(ctg) = line.strip_prefix('>') {
						let name = ctg.trim_end();
						if let Some(mut c) = current_ctg.take() {
							c.md5.set(&md5_hasher.finalize_reset())?;
							c.process_ctg(&cache_path, &min_contig_size, &mut md5_data).map_err(|e| format!("{}", e))?;
							if let Some(buf) = &mut md5_data.cache_buf { buf.clear() }
						}	
						current_ctg = Some(Md5Contig::new(name));									
					} else if let Some(ctg) = &mut current_ctg { 
						md5_buf.clear();
						line.as_bytes().iter().filter(|x| filt_tab[**x as usize] != 0).for_each(|x| md5_buf.push(*x));
						ctg.len += md5_buf.len();
						md5_hasher.update(&md5_buf);
						if let Some(buf) = &mut md5_data.cache_buf { buf.extend_from_slice(&md5_buf) }
					} else { return Err(format!("Error reading {}: no FASTA sequence header line", p.as_ref().display())) }
					if md5_data.cache_buf.is_none() { md5_data.output.write(line.as_bytes()).map_err(|e| format!("{}", e))?; }
					line.clear();
				},
				Err(e) => return Err(format!("Error reading from file {}: {}", p.as_ref().display(), e)),
			}
		}
		if let Some(mut c) = current_ctg.take() {
			c.md5.set(&md5_hasher.finalize_reset())?;
			c.process_ctg(&cache_path, &min_contig_size, &mut md5_data).map_err(|e| format!("{}", e))?;
			if let Some(buf) = &mut md5_data.cache_buf { buf.clear() }
		}
		debug!("File processed in {}ms", now.elapsed().as_millis());
		// Only apply contig size limit to first file (main reference file)
		min_contig_size = None;
	}
	Ok(())
}

fn hex_val(x: u8) -> Result<u8, String> {
	match x {
		b'a'..=b'f' => Ok(x - b'a' + 10),
		b'A'..=b'F' => Ok(x - b'A' + 10),
		b'0'..=b'9' => Ok(x - b'0'),
		_ => Err(format!("Unrecognized hex digit {}", x)),
	}
}

fn get_from_hex<T: AsRef<[u8]>>(v: T) -> Result<Vec<u8>, String> {
	let v = v.as_ref();
	if v.len() & 1 != 0 { return Err("Couldn't convert from hex - odd number of digits".to_string()); }
	v.chunks(2).map(|v| Ok(hex_val(v[0])? << 4 | hex_val(v[1])?)).collect()
}

// Check if reference cache is populated and populate with any contigs not present in the cache.
pub fn check_reference_cache<P: AsRef<Path>, Q: AsRef<Path>>(gem_bs: &GemBS, gref: P, ctg_md5: Q) -> Result<(), String> {
	let mut rdr = compress::open_bufreader(ctg_md5.as_ref()).map_err(|x| format!("{}",x))?;
	debug!("Checking reference cache");
	let cp = get_cache_path();
	let now = Instant::now();
	let mut line = String::with_capacity(1024);
	loop {
		gem_bs.check_signal()?;
		match rdr.read_line(&mut line) {
			Ok(0) => break,
			Ok(_) => {
				let mut iter = line.split_ascii_whitespace();
				let e = if let Some(name) = iter.next() {
					let mut found = false;
					for f in iter {
						if f.starts_with("M5:") && f.len() >= 35 {
							let md5 = get_from_hex(&f.as_bytes()[3..])?;
							let mut c = Md5Contig::new(&name);
							c.md5.set(&md5)?;
							c.check_cache(gem_bs, &cp, &gref)?;
							found = true;
							break;
						}
					}
					!found
				} else { true };
				if e  { return Err(format!("Error reading from file {}", ctg_md5.as_ref().display())) }
				line.clear();
			},
			Err(e) => return Err(format!("Error reading from file {}: {}", ctg_md5.as_ref().display(), e)),
		}
	}
	debug!("Reference cache checked in {}ms", now.elapsed().as_millis());		

	Ok(())
}
use utils::compress;
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{fmt, env, fs};
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
			Err("Md5Digest::set requires [u8; 16]".to_string())
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
	fn handle_cache(&self, cache: &mut RefCache) -> Result<(), String> {
		let fname = cache.get_fname(format!("{}", self.md5).as_str())?;
		if !fname.exists() {
			// Make cache directories if required
			if let Some(d) = &fname.parent() { fs::create_dir_all(d).map_err(|e| format!("Couldn't create cache directory {}: {}", d.display(), e))? }
			let mut wrt = compress::open_bufwriter(&fname).map_err(|e| format!("Couldn't open cache file {} for writing: {}", &fname.display(), e))?;
			wrt.write_all(&cache.buf).map_err(|e| format!("Error writing to cache file {}: {}", &fname.display(), e))?;			
		}
		cache.buf.clear();
		Ok(())
	}
//	pub fn name(&self) -> &str { &self.name }
//	pub fn len(&self) -> usize { self.len }
}

struct RefCache {
	path_str: String,
	buf: Vec<u8>,	
}

impl RefCache {
	// Replace %s or %ds where d is an integer by sections of the md5 string to make a filename
	fn get_fname(&self, md5: &str) -> Result<PathBuf, String> {
		lazy_static! { static ref RE: Regex = Regex::new(r"([^%]*)%(\d*)s([^%]*)").unwrap(); }
		let mut fname = String::new();
		let mut ix = 0;
		for c in RE.captures_iter(&self.path_str) {
			fname.push_str(&c[1]);
			let n = if c[2].is_empty() {
				32 - ix	
			} else if let Ok(x) = <usize>::from_str(&c[2]) {
				if x + ix > 32 { 32 - ix } else { x }
			} else { 
				return Err(format!("Illegal format string for cache path: {}", &self.path_str)) 
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
}

pub fn md5_fasta<P: AsRef<Path>, Q: AsRef<Path>, R: AsRef<Path>>(gem_bs: &GemBS, in_files: &[P], output_ref: Q, ctg_md5: R) -> Result<(), String> {
	let mut cache = if gem_bs.get_config_bool(Section::Index, "populate_cache") { 
		let cp = get_cache_path();
		debug!("reference cache_path = {}", cp);
		Some(RefCache{path_str: cp, buf: Vec::with_capacity(16384)})
	} else { None };
	let opath = output_ref.as_ref();
	let threads = gem_bs.get_threads(Section::Index);
	let bgzip_path = gem_bs.get_exec_path("bgzip");
	let filt_tab = init_filter();
	let mut output = compress::open_pipe_writer(opath, &bgzip_path, &["-@", format!("{}", threads).as_str()])
		.map_err(|e| format!("Couldn't open output {}: {}", opath.display(), e))?;
	let mut output_md5 = compress::open_bufwriter(ctg_md5.as_ref())
		.map_err(|e| format!("Couldn't open output {}: {}", ctg_md5.as_ref().display(), e))?;
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
							writeln!(output_md5, "{}", c).map_err(|e| format!("{}", e))?;
							if let Some(r) = &mut cache { c.handle_cache(r)?; }
						}	
						current_ctg = Some(Md5Contig::new(name));									
					} else if let Some(ctg) = &mut current_ctg { 
						md5_buf.clear();
						line.as_bytes().iter().filter(|x| filt_tab[**x as usize] != 0).for_each(|x| md5_buf.push(*x));
						ctg.len += md5_buf.len();
						md5_hasher.update(&md5_buf);
						if let Some(r) = &mut cache { r.buf.extend_from_slice(&md5_buf) }
					} else { return Err(format!("Error reading {}: no FASTA sequence header line", p.as_ref().display())) }
					output.write(line.as_bytes()).map_err(|e| format!("{}", e))?;
					line.clear();
				},
				Err(e) => return Err(format!("Error reading from file {}: {}", p.as_ref().display(), e)),
			}
		}
		if let Some(mut c) = current_ctg.take() {
			c.md5.set(&md5_hasher.finalize_reset())?;
			writeln!(output_md5, "{}", c).map_err(|e| format!("{}", e))?;
			if let Some(r) = &mut cache { c.handle_cache(r)?; }
		}	
		debug!("File processed in {}ms", now.elapsed().as_millis());
	}
	Ok(())
}

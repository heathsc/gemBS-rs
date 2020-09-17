use std::{env, fmt};
use std::collections::HashMap;
use std::io::{self, Write, StdinLock, StdoutLock, BufRead, Error, ErrorKind};
use lazy_static::lazy_static;

use utils::compress;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TagType { SN, AS, M5, SP, LN }

impl fmt::Display for TagType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			TagType::SN => write!(f, "SN"),
			TagType::AS => write!(f, "AS"),
			TagType::M5 => write!(f, "M5"),
			TagType::SP => write!(f, "SP"),
			TagType::LN => write!(f, "LN"),
		}
	}
}

struct Contig {
	tags: HashMap<TagType, String>
}

fn read_contig_file(fname: &str, href: &mut HashMap<String, Contig>) -> io::Result<()> {
	let mut rdr = compress::open_bufreader(fname)?;
	let mut buf = String::new();
	loop {
		match rdr.read_line(&mut buf) {
			Ok(0) => break,
			Ok(_) => {
				let mut iter = buf.trim_end().split('\t');
				if let Some(name) = iter.next() {
					let mut tags = HashMap::new();
					for s in iter {
						match &s[..3] {
							"LN:" => { tags.insert(TagType::LN, s[3..].to_owned()); },
							"M5:" =>  { tags.insert(TagType::M5, s[3..].to_owned()); },
							"AS:" =>  { tags.insert(TagType::AS, s[3..].to_owned()); },
							"SP:" =>  { tags.insert(TagType::SP, s[3..].to_owned()); },
							_ => (),
						}
					}
					let ctg = Contig{tags};
					href.insert(name.to_string(), ctg);
				}
				buf.clear();
			}
			Err(e) => return Err(e),
		}
	}	
	Ok(())
}
fn handle_header(rd_handle: &mut StdinLock, wr_handle: &mut StdoutLock) -> io::Result<Vec<u8>> {
	let args: Vec<String> = env::args().collect();
	let mut contig_hash = HashMap::new();
	if args.len() > 1 { read_contig_file(&args[1], &mut contig_hash)? }
    let mut buffer = String::new();
	loop {
		match rd_handle.read_line(&mut buffer) {
			Ok(0) => {
				buffer.clear();
				break;
			},
			Ok(_) => {
				if !buffer.starts_with('@') { break; }
				if buffer.starts_with("@SQ\t") {
					let mut iter = buffer.trim_end().split('\t');
					iter.next();
					let mut tags = HashMap::new();
					let mut gen_tags = Vec::new();
					for s in iter {
						match &s[..3] {
							"SN:" => { tags.insert(TagType::SN, &s[3..]); },
							"LN:" => { tags.insert(TagType::LN, &s[3..]); },
							"M5:" =>  { tags.insert(TagType::M5, &s[3..]); },
							"AS:" =>  { tags.insert(TagType::AS, &s[3..]); },
							"SP:" =>  { tags.insert(TagType::SP, &s[3..]); },
							_ => gen_tags.push(s),
						}
					}
					if let Some(name) = tags.get(&TagType::SN) {
						if let Some(hr) = contig_hash.get(&name.to_string()) {
							for (tag, s) in hr.tags.iter() { tags.insert(*tag, s); }
						}
						write!(wr_handle, "@SQ")?;
						for t in &[TagType::SN, TagType::LN, TagType::M5, TagType::AS, TagType::SP] {
							if let Some(s) = tags.get(t) { write!(wr_handle, "\t{}:{}", t, s)?; }
						}
						for gt in gen_tags { write!(wr_handle, "\t{}", gt)?; }
						writeln!(wr_handle)?;
					} else { return Err(Error::new(ErrorKind::Other, "No SN tag in @SQ Header line")) }
				} else {
					wr_handle.write_all(buffer.as_bytes())?;
				}	
				buffer.clear();
			},
			Err(e) => return Err(e),
		}
	}
	let buf = buffer.as_bytes().iter().fold(Vec::new(), |mut v, c| {v.push(*c); v});
	Ok(buf)
} 

lazy_static! { 
	static ref FTAB: [u8; 256] = {
		let mut v = [b'_'; 256];
		for i in b'!'..=b'~' { v[i as usize] = i }
		v[b'@' as usize] = b'_';
		v
	};
}

fn clean_readname(s: &mut [u8]) {
	for c in s.iter_mut() {
		if *c == b'\t' { break; }
		*c = FTAB[*c as usize];
	} 
}


fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut rd_handle = stdin.lock();
    let stdout = io::stdout();
    let mut wr_handle = stdout.lock();
	let mut buffer = handle_header(&mut rd_handle, &mut wr_handle)?;
	if buffer.is_empty() { return Ok(()) }
	loop {
		clean_readname(&mut buffer);
		wr_handle.write_all(&buffer)?;
		buffer.clear();
		match rd_handle.read_until(b'\n', &mut buffer) {
			Ok(0) => { break; },
			Ok(_) => (),
			Err(e) => return Err(e),
		}		
	}

    Ok(())
}

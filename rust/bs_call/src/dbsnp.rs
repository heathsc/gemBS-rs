use std::{fs, io};
use std::io::{Error, ErrorKind, Read, BufReader, BufRead, Seek, SeekFrom};
use std::convert::TryInto;
use libflate::zlib::Decoder;
use std::collections::HashMap;

pub fn new_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

pub struct DBSnpBin {
	mask: u64,
	fq_mask: u64,
	entries: Box<[u16]>,
	name_buf: Box<[char]>,
}

pub struct DBSnpCtg {
//	name: String,
	min_bin: usize,
	max_bin: usize,
	file_offset: u64,
	bins: Vec<DBSnpBin>,
}

pub struct DBSnpIndex {
	filename: String,
	file: Box<dyn BufRead>,
	dbsnp: HashMap<String, DBSnpCtg>,
	prefixes: Vec<String>,
	header: String,	
}

impl DBSnpIndex {
	pub fn new<S: AsRef<str>>(name: S) -> io::Result<Self> {
		let filename = name.as_ref();
		let mut file = BufReader::new(fs::File::open(filename)?);
		debug!("Reading dbSNP header from {}", filename);
		let mut td = [0u32; 2];
		read_u32(&mut file, &mut td)?;
		if td[0] != 0xd7278434 { return Err(new_err("Invalid format: bad magic number".to_string())) }
		trace!("Magic number OK");
		let mut td1 = [0u64; 3];
		read_u64(&mut file, &mut td1)?;
		file.seek(SeekFrom::Start(td1[0]))?;
		let cbuf = read_n(&mut file, td1[2] as usize)?;
		read_u32(&mut file, &mut[td[0]])?;
		if td[0] != 0xd7278434 { return Err(new_err("Invalid format: bad second magic number".to_string())) }
		trace!("Header data read in OK");
		let mut decoder = Decoder::new(&*cbuf)?;
		let mut ubuf = Vec::new();
		decoder.read_to_end(&mut ubuf)?;
		trace!("Header data uncompressed OK");
		if ubuf.len() < 9 { return Err(new_err("Invalid format: short header".to_string())) }
		let n_prefix = u16::from_le_bytes((&ubuf[2..4]).try_into().unwrap());
		let n_ctgs = u32::from_le_bytes((&ubuf[4..8]).try_into().unwrap());
		let (header, mut p) = get_string(&ubuf[8..])?;
		let mut prefixes = Vec::with_capacity(n_prefix as usize);
		for _ in 0..n_prefix { 
			let (s, p1) = get_string(p)?;
			p = p1;
			prefixes.push(s);
		}
		trace!("Prefixes read in OK");
		let mut dbsnp = HashMap::new();
		for _ in 0..n_ctgs {
			let ctg = get_ctg_header(p)?;
			let (s, p1) = get_string(&p[16..])?;
			p = p1;
			dbsnp.insert(s, ctg);
		}
		trace!("Contigs read in OK");
		info!("Read dbSNP header from {} with data on {} contigs", filename, n_ctgs);
		info!("Header line: {}", header);
		Ok(Self{filename: filename.to_owned(), file: Box::new(file), dbsnp, prefixes, header})
	}
}

fn get_ctg_header(buf: &[u8]) -> io::Result<DBSnpCtg> {
	if buf.len() > 16 {
		let min_bin = u32::from_le_bytes((&buf[0..4]).try_into().unwrap()) as usize;
		let max_bin = u32::from_le_bytes((&buf[4..8]).try_into().unwrap()) as usize;
		let file_offset = u64::from_le_bytes((&buf[8..16]).try_into().unwrap());
		Ok(DBSnpCtg{min_bin, max_bin, file_offset, bins: Vec::new()})
	} else {
		Err(new_err("Bad format: Failed to read in contig header".to_string()))
	}
}

fn get_string(buf: &[u8]) -> io::Result<(String, &[u8])> {
	if buf.len() > 1 {
		let mut s = String::new();
		for (i, c) in buf.iter().copied().enumerate() {
			if c == 0 { 
				return Ok((s, &buf[i + 1..]));
			} else { s.push(c as char) }
		}
	}
	Err(new_err("Bad format: String terminator not found".to_string()))
}

fn read_n<R: Read>(reader: R, n: usize) -> io::Result<Vec<u8>> {
	let mut buf = vec![];
    let mut chunk = reader.take(n as u64);
    let n_read = chunk.read_to_end(&mut buf)?;
	if n == n_read { Ok(buf) } else { Err(new_err("Unexpected number of bytes read".to_string())) }
}

fn read_u32<R: Read>(mut file: R, buf: &mut[u32]) -> io::Result<()> {
	let mut p = [0u8; 4];
	for x in buf.iter_mut() {
		let n = file.read(&mut p)?;
		if n != 4 { return Err(new_err("Error reading from file".to_string())) }
		*x = u32::from_le_bytes(p);
	}
	Ok(())
}

fn read_u64<R: Read>(mut file: R, buf: &mut[u64]) -> io::Result<()> {
	let mut p = [0u8; 8];
	for x in buf.iter_mut() {
		let n = file.read(&mut p)?;
		if n != 8 { return Err(new_err("Error reading from file".to_string())) }
		*x = u64::from_le_bytes(p);
	}
	Ok(())
}

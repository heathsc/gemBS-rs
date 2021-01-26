use std::{fs, io};
use std::fs::File;
use std::sync::Arc;
use std::io::{Error, ErrorKind, Read, BufReader, Seek, SeekFrom};
use std::convert::TryInto;
use std::collections::HashMap;

use zstd::block::decompress_to_buffer;

pub struct DBSnpFile {
	file: BufReader<File>,
	index: DBSnpIndex,
}

impl DBSnpFile {
	pub fn open(index: DBSnpIndex) -> io::Result<Self> {
		let file = BufReader::new(fs::File::open(&index.filename)?);
		Ok(Self{file, index})
	}
	pub fn unload_ctg<S: AsRef<str>>(&mut self, name: S) {
		let name = name.as_ref();
		if let Some(ctg) = self.index.dbsnp.get_mut(name) {
			if ctg.ctg.bins.is_some() { 
				info!("Unloading dbSNP data for {}", name);
				let _ = ctg.ctg.bins.take();
			} else { warn!("Couldn't unload dbSNP data for {}: no data present", name) }
		}
	}
	
	pub fn load_ctg<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> {
		let name = name.as_ref();
		match self.index.dbsnp.get_mut(name) {
			Some(ctg) => {
				info!("Loading dbSNP data for {}", name);	
				ctg.load_data(&mut self.file, self.index.bufsize)?;			
				info!("dbSNP data loaded");			
			},
			None => {
				warn!("No dbSNP information found for {}", name);				
			},
		}
		Ok(())
	}
	pub fn get_dbsnp_contig<S: AsRef<str>>(&self, name: S) -> Option<DBSnpContig> {
		if let Some(ctg) = self.index.dbsnp.get(name.as_ref()) { Some(ctg.ctg.clone()) } else { None }
	}
}

#[derive(Clone)]
pub struct DBSnpContig {
	min_bin: usize,
	max_bin: usize,
	bins: Option<Arc<Vec<Option<DBSnpBin>>>>,	
}

impl DBSnpContig {
	pub fn lookup_rs(&self, x: usize) -> Option<(String, bool)> {
		if let Some(bins) = &self.bins {
			let bn = (x + 1) >> 8;
			if bn >= self.min_bin && bn <= self.max_bin {
				if let Some(bin) = &bins[bn - self.min_bin] { return bin.lookup_rs((x + 1) & 255) }
			}
		}
		None
	}
}

pub struct DBSnpIndex {
	filename: String,
	dbsnp: HashMap<String, DBSnpCtg>,
	bufsize: usize,
	header: String,	
}

impl DBSnpIndex {
	pub fn new<S: AsRef<str>>(name: S) -> io::Result<Self> {
		let filename = name.as_ref();
		let mut file = BufReader::new(fs::File::open(filename)?);
		debug!("Reading dbSNP header from {}", filename);
		let mut td = [0u32; 1];
		read_u32(&mut file, &mut td)?;
		if td[0] != 0xd7278434 { return Err(new_err(format!("Invalid format: bad magic number {:x}",td[0]))) }
		trace!("Magic number OK");
		let vs = read_n(&mut file, 4)?;
		if vs[0] != 2 { return Err(new_err("Invalid version number".to_string())) }
		let mut td1 = [0u64; 3];
		read_u64(&mut file, &mut td1)?;
		file.seek(SeekFrom::Start(td1[0]))?;
		let cbuf = read_n(&mut file, td1[2] as usize)?;
		read_u32(&mut file, &mut td)?;
		if td[0] != 0xd7278434 { return Err(new_err("Invalid format: bad second magic number".to_string())) }
		trace!("Header data read in OK");
		let mut ubuf: Vec<u8> = vec!(0; td1[1] as usize);
		let sz = decompress_to_buffer(&cbuf, &mut ubuf)?;
		trace!("Header data uncompressed OK ({} bytes)", sz);
		if sz < 4 { return Err(new_err("Invalid format: short header".to_string())) }
		let n_ctgs = u32::from_le_bytes((&ubuf[0..4]).try_into().unwrap());
		let mut p = &ubuf[4..sz];
		let mut dbsnp = HashMap::new();
		let mut ctgs = Vec::with_capacity(n_ctgs as usize);
		for _ in 0..n_ctgs {
			ctgs.push(get_ctg_header(p)?);
			p = &p[16..];
		}
		let (header, mut p) = get_string(p)?;
		for ctg in ctgs.drain(..) {
			let (s, p1) = get_string(p)?;
			p = p1;
			trace!("Inserting ctg {} {}-{}", s, ctg.min_bin(), ctg.max_bin());
			dbsnp.insert(s, ctg);			
		}
		if !p.is_empty() { Err(new_err("Error with dbSNP index header - excess data".to_string())) } 
		else {
			trace!("Contigs read in OK");
			info!("Read dbSNP header from {} with data on {} contigs", filename, n_ctgs);
			info!("Header line: {}", header);
			Ok(Self{filename: filename.to_owned(), dbsnp, bufsize: td1[1] as usize, header})
		}
	}
	pub fn header(&self) -> &str { &self.header }
}

/// 
/// Everything below is private to the module
/// 

fn new_err(s: String) -> io::Error {
	Error::new(ErrorKind::Other, s)	
}

struct DBSnpBin {
	mask: [u128; 2],
	name_len: Box<[u8]>,
	name_buf: Box<[u8]>,
}

const DTAB: [char; 16] = [ '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '?', '?', '?', '?', '?', '?'];

const WTAB: [&str; 256] = [
	"00", "01", "02", "03", "04", "05", "06", "07", "08", "09", "0?", "0?", "0?", "0?", "0", "0",
	"10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "1?", "1?", "1?", "1?", "1", "1",
	"20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "2?", "2?", "2?", "2?", "2", "2",
	"30", "31", "32", "33", "34", "35", "36", "37", "38", "39", "3?", "3?", "3?", "3?", "3", "3",
	"40", "41", "42", "43", "44", "45", "46", "47", "48", "49", "4?", "4?", "4?", "4?", "4", "4",
	"50", "51", "52", "53", "54", "55", "56", "57", "58", "59", "5?", "5?", "5?", "5?", "5", "5",
	"60", "61", "62", "63", "64", "65", "66", "67", "68", "69", "6?", "6?", "6?", "6?", "6", "6",
	"70", "71", "72", "73", "74", "75", "76", "77", "78", "79", "7?", "7?", "7?", "7?", "7", "7",
	"80", "81", "82", "83", "84", "85", "86", "87", "88", "89", "8?", "8?", "8?", "8?", "8", "8",
	"90", "91", "92", "93", "94", "95", "96", "97", "98", "99", "9?", "9?", "9?", "9?", "9", "9",
	"?0", "?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8", "?9", "??", "??", "??", "??", "?", "?",
	"?0", "?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8", "?9", "??", "??", "??", "??", "?", "?",
	"?0", "?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8", "?9", "??", "??", "??", "??", "?", "?",
	"?0", "?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8", "?9", "??", "??", "??", "??", "?", "?",
	"?0", "?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8", "?9", "??", "??", "??", "??", "?", "?",
	"?0", "?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8", "?9", "??", "??", "??", "??", "?", "?",	
];

impl DBSnpBin {
	fn lookup_rs(&self, ix: usize) -> Option<(String, bool)> {
		let (k, mk) = if ix < 128 { (0, 1u128 << ix) } else { (1, 1u128 << (ix & 127)) };
		if (self.mask[k] & mk) != 0 {
			let n_prev_entries = if k == 0 {
				(self.mask[k] & (mk - 1)).count_ones() as usize
			} else {
				(self.mask[0].count_ones() + (self.mask[1] & (mk - 1)).count_ones()) as usize
			};
			let start_x: usize = self.name_len[0..n_prev_entries].iter().map(|x| *x as usize).sum();
			let mut rs = String::with_capacity(self.name_len[n_prev_entries] as usize + 2);
			rs.push_str("rs");
			let mut it = self.name_buf[start_x>>1..].iter(); 
			if (start_x & 1) != 0 {	rs.push(DTAB[(it.next().expect("Short name buf") & 0xf) as usize]) }
			let select = loop {
				let c = it.next().expect("Short name buf");
				if (c & 0xf0) >= 0xe0 { break (c & 0xf0) == 0xf0 }
				if (c & 0xf) > 9 && (c & 0xf) < 0xe { println!("OOOK! {:x}", c)};
				rs.push_str(WTAB[*c as usize]);
				if (c & 0xf) >= 0xe { break (c & 0xf) == 0xf }
			};
			Some((rs, select))
		} else {
			None
			
		}
	} 
}

struct DBSnpCtg {
	ctg: DBSnpContig,
	file_offset: u64,
}

impl DBSnpCtg {
	fn min_bin(&self) -> usize { self.ctg.min_bin }
	fn max_bin(&self) -> usize { self.ctg.max_bin }
	fn load_data(&mut self, mut file: &mut BufReader<File>, bufsize: usize) -> io::Result<()> {
		file.seek(SeekFrom::Start(self.file_offset))?;
		let mut bins = Vec::with_capacity(self.max_bin() + 1 - self.min_bin());
		let mut ubuf: Vec<u8> = vec!(0; bufsize);
		loop {
			let mut size = [0u64; 1];
			read_u64(&mut file, &mut size)?;
			if size[0] == 0 { break; }
			let mut tt = [0u32; 1];
			read_u32(&mut file, &mut tt)?;
			let first_bin = tt[0] as usize;	
			if first_bin < self.min_bin() + bins.len() { return Err(new_err("Error: index data corrupt".to_string())) }	
			let gap = first_bin - self.min_bin() - bins.len();
			let cbuf = read_n(&mut file, size[0] as usize)?;
			trace!("Read in compressed data for bin");
			let sz = decompress_to_buffer(&cbuf, &mut ubuf)?;
			trace!("bin data uncompressed OK");	
			bins.append(&mut load_bins(&ubuf[..sz], gap)?);				
		}
		if bins.len() != self.max_bin() + 1 - self.min_bin() { Err(new_err(format!("Wrong number of bins read in.  Expected {}, Found {}", self.max_bin() + 1 - self.min_bin(), bins.len()))) }
		else {
			let n_snps = bins.iter().fold(0, |s, b| if let Some(bin) = b {s + bin.name_len.len()} else { s } );
			info!("Contig dbSNP data read: number of snps = {}", n_snps);
			self.ctg.bins = Some(Arc::new(bins));
			Ok(())
		}
	}
}

fn load_bins(mut buf: &[u8], gap: usize) -> io::Result<Vec<Option<DBSnpBin>>> {
	let format_err = || Err(new_err("Format error".to_string()));
	let mut bins = Vec::with_capacity(256);	
	let mut first = true;
	let mut mask = [0u128; 2]; 
	let mut x16 = [0u16; 1];	
	let mut x32 = [0u32; 1];	
	loop {
		let bin_inc = if first {
			first = false; 
			gap
		} else {
			let x = match read_1(&mut buf) {
				Ok(x) => x,
				Err(e) if e.kind() == ErrorKind::UnexpectedEof =>  break,
				Err(e) => return Err(e),
			};
			match x & 192 {
				0 => x as usize,
				64 => read_1(&mut buf)? as usize,
				128 => {
					read_u16(&mut buf, &mut x16)?;
					x16[0] as usize
				},
				_ => {
					read_u32(&mut buf, &mut x32)?;
					x32[0] as usize
				},
			}
		};
		for _ in 0..bin_inc { bins.push(None) }
		read_u128(&mut buf, &mut mask)?;
		let n = mask[0].count_ones() + mask[1].count_ones();
		if n == 0 { return format_err() }
		let mut name_buf = Vec::new();
		let mut name_len = Vec::with_capacity(n as usize);
		let mut len = 0;
		let mut ix = 0;
		loop {
			let x = read_1(&mut buf)?;
			name_buf.push(x);
			if len == 255 { return format_err() }
			len += 1;
			if (x & 0xf0) >= 0xe0 {
				name_len.push(len);
				ix += 1;
				if ix == n { break; } 
				len = 0;
			}
			if len == 255 { return format_err() } 
			len += 1;
			if (x & 0xf) >= 0xe {
				name_len.push(len);				
				ix += 1;
				if ix == n { break; }
				len = 0; 
			}
		}
		bins.push(Some(DBSnpBin{mask, name_buf: name_buf.into_boxed_slice(), name_len: name_len.into_boxed_slice()}));
	}
	Ok(bins)
}	


fn get_ctg_header(buf: &[u8]) -> io::Result<DBSnpCtg> {
	if buf.len() >= 16 {
		let min_bin = u32::from_le_bytes((&buf[0..4]).try_into().unwrap()) as usize;
		let max_bin = u32::from_le_bytes((&buf[4..8]).try_into().unwrap()) as usize;
		let file_offset = u64::from_le_bytes((&buf[8..16]).try_into().unwrap());
		let ctg = DBSnpContig{min_bin, max_bin, bins: None};
		Ok(DBSnpCtg{ctg, file_offset})
	} else {
		Err(new_err("Bad format: Failed to read in contig header".to_string()))
	}
}

fn get_string(buf: &[u8]) -> io::Result<(String, &[u8])> {
	if !buf.is_empty() {
		let mut s = String::new();
		for (i, c) in buf.iter().copied().enumerate() {
			if c == 0 { 
				return Ok((s, &buf[i + 1..]));
			} else { s.push(c as char) }
		}
	}
	Err(new_err("Bad format: String terminator not found".to_string()))
}

fn read_1<R: Read>(reader: &mut R) -> io::Result<u8> {
	let mut x = [0u8; 1];
	reader.read_exact(&mut x).map(|_| x[0])
}

fn read_n<R: Read>(reader: &mut R, n: usize) -> io::Result<Vec<u8>> {
	let mut buf = vec![];
    let mut chunk = reader.take(n as u64);
    let n_read = chunk.read_to_end(&mut buf)?;
	if n == n_read { Ok(buf) } else { Err(new_err("Unexpected number of bytes read".to_string())) }
}

fn read_u16<R: Read>(file: &mut R, buf: &mut[u16]) -> io::Result<()> {
	let mut p = [0u8; 2];
	for x in buf.iter_mut() {
		file.read_exact(&mut p)?;
		*x = u16::from_le_bytes(p);
	}
	Ok(())
}

fn read_u32<R: Read>(file: &mut R, buf: &mut[u32]) -> io::Result<()> {
	let mut p = [0u8; 4];
	for x in buf.iter_mut() {
		file.read_exact(&mut p)?;
		*x = u32::from_le_bytes(p);
	}
	Ok(())
}

fn read_u64<R: Read>(file: &mut R, buf: &mut[u64]) -> io::Result<()> {
	let mut p = [0u8; 8];
	for x in buf.iter_mut() {
		file.read_exact(&mut p)?;
		*x = u64::from_le_bytes(p);
	}
	Ok(())
}

fn read_u128<R: Read>(file: &mut R, buf: &mut[u128]) -> io::Result<()> {
	let mut p = [0u8; 16];
	for x in buf.iter_mut() {
		file.read_exact(&mut p)?;
		*x = u128::from_le_bytes(p);
	}
	Ok(())
}

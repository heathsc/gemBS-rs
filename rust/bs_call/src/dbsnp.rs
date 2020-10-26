use std::{fs, io};
use std::fs::File;
use std::sync::Arc;
use std::io::{Error, ErrorKind, Read, BufReader, Seek, SeekFrom};
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
	name_buf: Box<[u8]>,
}

const DTAB: [char; 16] = [ '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ' ', ' ', ' ', ' ', ' ', ' '];

impl DBSnpBin {
	pub fn lookup_rs(&self, ix: usize, prefixes: &Arc<Vec<String>>) -> Option<(String, bool)> {
		let mk = 1u64 << ix;
		if (self.mask & mk) != 0 {
			let res = (self.fq_mask & mk) != 0;
			let mut rs = String::new();
			let n_prev_entries = (self.mask & (mk - 1)).count_ones();
			let mut entries = self.entries.iter().copied();
			let mut ix = 0;
			for _ in 0..n_prev_entries {
				let en = entries.next().unwrap() as usize;
				ix += (en >> 8) + if (en >> 6).trailing_zeros() >= 2 { 2 } else { 0 };				
			}
			let en = entries.next().unwrap() as usize;
			let ix1 = ix + (en >> 8) as usize;
			let mut nm = self.name_buf[ix..ix1].iter().copied();
			rs.push_str(&prefixes [{
				let t = (en >> 6) & 3;
				if t == 0 {
					((nm.next().unwrap() as usize) << 8) | (nm.next().unwrap() as usize)
				} else { t - 1 }
			}]);
			for c in nm {
				rs.push(DTAB[(c >> 4) as usize]);
				if (c & 15) < 10 { rs.push(DTAB[(c & 15) as usize]) };
			}
			Some((rs, res))
		} else {
			None
		}
	}
}

pub struct TempBin {
	mask: u64,
	fq_mask: u64,
	entries: Vec<u16>,
	name_buf: Vec<u8>,	
	prev_ix: Option<usize>,
}

impl TempBin {
	fn new() -> Self { Self {mask: 0, fq_mask: 0, entries: Vec::new(), name_buf: Vec::new(), prev_ix: None }}
	fn convert_to_bin(self) -> DBSnpBin {
		DBSnpBin { mask: self.mask, fq_mask: self.fq_mask, entries: self.entries.into_boxed_slice(), name_buf: self.name_buf.into_boxed_slice() }
	}
	fn is_empty(&self) -> bool { self.entries.is_empty() }
}

pub struct DBSnpCtg {
	min_bin: usize,
	max_bin: usize,
	file_offset: u64,
	bins: Option<Arc<Vec<Option<DBSnpBin>>>>,
}

impl DBSnpCtg {
	fn load_data(&mut self, mut file: &mut BufReader<File>, prefixes: &[String]) -> io::Result<()> {
		file.seek(SeekFrom::Start(self.file_offset))?;
		let mut bins = Vec::with_capacity(self.max_bin + 1 - self.min_bin);
		let mut ubuf = Vec::new();
		loop {
			let mut size = [0u64; 1];
			read_u64(&mut file, &mut size)?;
			if size[0] == 0 { break; }
			let cbuf = read_n(&mut file, size[0] as usize)?;
			trace!("Read in compressed data for bin");
			let mut decoder = Decoder::new(&*cbuf)?;
			ubuf.clear();
			decoder.read_to_end(&mut ubuf)?;
			trace!("bin data uncompressed OK");	
			bins.append(&mut load_bins(&ubuf, self.max_bin + 1 - self.min_bin - bins.len(), prefixes)?);				
		}
		if bins.len() != self.max_bin + 1 - self.min_bin { Err(new_err(format!("Wrong number of bins read in.  Expected {}, Found {}", self.max_bin + 1 - self.min_bin, bins.len()))) }
		else {
			let n_snps = bins.iter().fold(0, |s, b| if let Some(bin) = b {s + bin.entries.len()} else { s } );
			info!("Contig dbSNP data read: number of snps = {}", n_snps);
			self.bins = Some(Arc::new(bins));
			Ok(())
		}
	}
}

const DB_TAB: [u8; 256]	= [
	0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
	0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
	0xff, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x10, 0x11, 0x12, 0x13, 0x14,
	0x15, 0x16, 0x17, 0x18, 0x19, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x30,
	0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46,
	0x47, 0x48, 0x49, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x60, 0x61, 0x62,
	0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78,
	0x79, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x90, 0x91, 0x92, 0x93, 0x94,
	0x95, 0x96, 0x97, 0x98, 0x99, 0x0f, 0x1f, 0x2f, 0x3f, 0x4f, 0x5f, 0x6f, 0x7f, 0x8f, 0x9f, 0xff,
	0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
	0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
	0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
	0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
	0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
	0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
	0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
];

fn load_bins(buf: &[u8], n_bins: usize, prefixes: &[String]) -> io::Result<Vec<Option<DBSnpBin>>> {
	let format_err = || Err(new_err("Format error".to_string()));
	let mut bins = Vec::with_capacity(n_bins);
	let mut tbin = TempBin::new();
	let mut b = buf.iter().copied();
	let mut get = || b.next().ok_or_else(|| new_err("Short bin entry".to_string()));
	loop {
		if tbin.is_empty() {
			let x = match get() {
				Ok(c) => c,
				Err(_) => break,
			};
			let bin_inc = match x & 3 {
				0 => (x >> 2) as usize,
				1 => get()? as usize,
				2 => u16::from_le_bytes([get()?, get()?]) as usize,
				_ => u32::from_le_bytes([get()?, get()?, get()?, get()?]) as usize,
			};
			if bin_inc + bins.len() > n_bins { error!("Too many bins"); return format_err() }
			if bin_inc > 1 { for _ in 0..bin_inc - 1 { bins.push(None) } }				
		}
		let x = get()?;
		let prefix_ix = (x >> 6) as usize;
		if prefix_ix == 0 {
			tbin.name_buf.push(get()?);
			tbin.name_buf.push(get()?);
		}
		let ix = (x & 63) as usize;
		if let Some(x) = tbin.prev_ix { if ix <= x {
			error!("index problem {} {}", x, ix); 
			return format_err() } 
		}
		if prefix_ix > prefixes.len() { error!("Bad prefix ix: {}", prefix_ix); return format_err() }
		tbin.prev_ix = Some(ix);
		let k = tbin.name_buf.len();
		let tm = loop {
			let t = get()?;
			if t <= 3 { break t } 
			tbin.name_buf.push(DB_TAB[t as usize])
		};
		let k = tbin.name_buf.len() - k;
		if k > 255 { error!("SNP name too long"); return format_err() }
		let msk = 1u64 << ix;	
		tbin.mask |= msk;
		if (tm & 2) == 2 { tbin.fq_mask |= msk }
		if tbin.entries.len() == 64 { error!("Too many bin entries"); return format_err() }
		tbin.entries.push(((k as u16) << 8) | (x as u16));
		if (tm & 1) == 1 { // End of bin
			bins.push(Some(tbin.convert_to_bin()));
			tbin = TempBin::new();
		}
	}		
	Ok(bins)
}	

pub struct DBSnpContig {
	min_bin: usize,
	max_bin: usize,
	prefixes: Arc<Vec<String>>,
	bins: Option<Arc<Vec<Option<DBSnpBin>>>>,	
}

impl DBSnpContig {
	pub fn lookup_rs(&self, x: usize) -> Option<(String, bool)> {
		if let Some(bins) = &self.bins {
			let bn = (x + 1) >> 6;
			if bn >= self.min_bin && bn <= self.max_bin {
				if let Some(bin) = &bins[bn - self.min_bin] { return bin.lookup_rs((x + 1) & 63, &self.prefixes) }
			}
		}
		None
	}
}
pub struct DBSnpIndex {
	filename: String,
	dbsnp: HashMap<String, DBSnpCtg>,
	prefixes: Arc<Vec<String>>,
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
		Ok(Self{filename: filename.to_owned(), dbsnp, prefixes: Arc::new(prefixes), header})
	}
}

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
			if ctg.bins.is_some() { 
				info!("Unloading dbSNP data for {}", name);
				let _ = ctg.bins.take();
			} else { warn!("Couldn't unload dbSNP data for {}: no data present", name) }
		}
	}
	
	pub fn load_ctg<S: AsRef<str>>(&mut self, name: S) -> io::Result<Option<&DBSnpCtg>> {
		let name = name.as_ref();
		Ok(match self.index.dbsnp.get_mut(name) {
			Some(ctg) => {
				info!("Loading dbSNP data for {}", name);	
				ctg.load_data(&mut self.file, &self.index.prefixes)?;			
				info!("dbSNP data loaded");
				Some(ctg)				
			},
			None => {
				warn!("No dbSNP information found for {}", name);
				None
			},
		})
	}
	pub fn get_ctg<S: AsRef<str>>(&self, name: S) -> Option<&DBSnpCtg> { self.index.dbsnp.get(name.as_ref()) }
	pub fn get_dbsnp_contig<S: AsRef<str>>(&self, name: S) -> Option<DBSnpContig> {
		if let Some(ctg) = self.get_ctg(name) {
			let bins = if let Some(b) = &ctg.bins { Some(Arc::clone(&b)) } else { None };
			let prefixes = Arc::clone(&self.index.prefixes);
			Some(DBSnpContig {
				min_bin: ctg.min_bin,
				max_bin: ctg.max_bin,
				prefixes, bins
			})
		} else { None }
	}
}

fn get_ctg_header(buf: &[u8]) -> io::Result<DBSnpCtg> {
	if buf.len() > 16 {
		let min_bin = u32::from_le_bytes((&buf[0..4]).try_into().unwrap()) as usize;
		let max_bin = u32::from_le_bytes((&buf[4..8]).try_into().unwrap()) as usize;
		let file_offset = u64::from_le_bytes((&buf[8..16]).try_into().unwrap());
		Ok(DBSnpCtg{min_bin, max_bin, file_offset, bins: None})
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

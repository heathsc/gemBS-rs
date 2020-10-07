use std::io;
use r_htslib::*;

pub struct VcfFile {
	pub file: HtsFile, 
	pub hdr: VcfHeader,
}

impl VcfFile {
	pub fn new<S: AsRef<str>>(name: S, mode: &str) -> io::Result<Self> {
		let name = name.as_ref();
		let file = HtsFile::new(name, mode)?;
		let hdr = VcfHeader::new("w")?;
		Ok(Self{file, hdr})
	}
	pub fn hdr(&mut self) -> &mut VcfHeader { &mut self.hdr }
	pub fn set_threads(&mut self, t: usize) -> io::Result<()> { self.file.set_threads(t) }
	pub fn write_hdr(&mut self) -> io::Result<()> { self.hdr.write(&mut self.file) }
}


use std::io;
use rust_htslib::htslib;

use crate::config::new_err;
use super::{get_cstr, from_cstr, HtsFile};

pub struct VcfHeader {
	inner: *mut htslib::bcf_hdr_t,
}

impl VcfHeader {
	pub fn new(mode: &str) -> io::Result<VcfHeader> {
		let hdr = unsafe{ htslib::bcf_hdr_init(get_cstr(mode).as_ptr()) };
		if hdr.is_null() { Err(new_err("Couldn't create VCF/BCF header".to_string())) }
		else { Ok(VcfHeader{inner: hdr})}
	}
	
	pub fn append<S: AsRef<str>>(&mut self, line: S) -> io::Result<()> {
		match unsafe{ htslib::bcf_hdr_append(self.inner, get_cstr(line).as_ptr())} {
			0 => Ok(()),
			_ => Err(new_err("Error appending line to VCF/BCF header".to_string()))
		}
	}
	
	pub fn get_version(&self) -> &str {
		from_cstr(unsafe { htslib::bcf_hdr_get_version(self.inner) })
	}
	
	pub fn add_sample<S: AsRef<str>>(&mut self, name: S) -> io::Result<()> {
		match unsafe{ htslib::bcf_hdr_add_sample(self.inner, get_cstr(name).as_ptr())} {
			0 => Ok(()),
			_ => Err(new_err("Error adding sample to VCF/BCF header".to_string()))
		}
	}	
	pub fn write(&self, hout: &mut HtsFile) -> io::Result<()> {
		match unsafe { htslib::bcf_hdr_write(hout.inner(), self.inner) } {
			0 => Ok(()),
			_ => Err(new_err("Error writing VCF/BCF header".to_string()))			
		}
	}
	pub fn id2int<S: AsRef<str>>(&self, category: usize, name: S) -> Option<usize> {
		let ix = unsafe { htslib::bcf_hdr_id2int(self.inner, category as libc::c_int, get_cstr(name.as_ref()).as_ptr()) };
		if ix < 0 { None } else { Some(ix as usize) }
	}
	pub fn sync(&mut self) -> io::Result<()> {
		match unsafe{ htslib::bcf_hdr_sync(self.inner)} {
			0 => Ok(()),
			_ => Err(new_err("Error adding sample to VCF/BCF header".to_string()))
		}		
	}
}

impl Drop for VcfHeader {
	fn drop(&mut self) {
		unsafe { htslib::bcf_hdr_destroy(self.inner) };
	}
}

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
	pub fn write_hdr(&mut self) -> io::Result<()> {
		match unsafe { htslib::bcf_hdr_write(self.file.inner(), self.hdr.inner) } {
			0 => Ok(()),
			_ => Err(new_err("Error writing VCF/BCF header".to_string()))			
		}
	}
}


use std::io::{self, ErrorKind, Error};
use std::ffi::c_void;


use libc::{c_int, c_uchar, off_t};
use c2rust_bitfields::BitfieldStruct;
use super::{hts_err, hFILE};

#[repr(C)]
pub struct bgzidx_t { _unused: [u8; 0], }
#[repr(C)]
#[derive(BitfieldStruct)]
pub struct BGZF {
	#[bitfield(name = "errcode", ty = "u16", bits = "0..=15")]
	#[bitfield(name = "reserved", ty = "c_uchar", bits = "16..=16")]
	#[bitfield(name = "is_write", ty = "c_uchar", bits = "17..=17")]
	#[bitfield(name = "no_eof_block", ty = "c_uchar", bits = "18..=18")]
	#[bitfield(name = "is_be", ty = "c_uchar", bits = "19..=19")]
	#[bitfield(name = "compress_level", ty = "u16", bits = "20..=28")]
	#[bitfield(name = "last_block_eof", ty = "c_uchar", bits = "29..=29")]
	#[bitfield(name = "is_compressed", ty = "c_uchar", bits = "30..=30")]
	#[bitfield(name = "is_gzip", ty = "c_uchar", bits = "31..=31")]
	bfield: [u8; 4],
	cache_size: c_int,
	block_length: c_int,
	block_clength: c_int,
	block_offset: c_int,
	block_address: i64,
	uncompressed_address: i64,
	uncompressed_block: *mut c_void,
	compressed_block: *mut c_void,
	cache: *mut c_void,
	fp: *mut hFILE,
	mt: *mut c_void,
	idx: *mut bgzidx_t,
	idx_build_otf: c_int,
	z_stream_s: *mut c_void,
	seeked: i64,
}

#[link(name = "hts")]
extern "C" {
	fn bgzf_seek(fp: *mut BGZF, pos: i64, whence: c_int) -> i64;
	fn bgzf_read_block(fp: *mut BGZF) -> c_int;
}

impl BGZF {
	pub fn tell(&self) -> i64 { (self.block_address << 16) | ((self.block_offset & 0xffff) as i64)}
	pub fn htell(&self) -> off_t {
		if self.mt.is_null() {
			let fp = unsafe{&*self.fp};
			fp.offset + (unsafe{fp.begin.offset_from(fp.buffer)} as off_t)
		} else { panic!("Multithreaded htell() not supported")}	
	}
	pub fn seek(&mut self, pos: i64) -> i64 {unsafe{bgzf_seek(self, pos, libc::SEEK_SET as c_int)}}
	pub fn getline(&mut self, delim: u8, s: &mut Vec<u8>) -> io::Result<usize> {
		s.clear();
		let mut state = 0;
		loop {
			if self.block_offset >= self.block_length {
				if unsafe{bgzf_read_block(self)} != 0 {
					state = -2;
					break;
				}
				if self.block_length == 0 {
					state = -1;
					break;
				}
			}
			let buf = unsafe{std::slice::from_raw_parts(self.uncompressed_block as *const u8, self.block_length as usize)};
			let mut l = 0;
			for c in &buf[self.block_offset as usize..] {
				l += 1;
				if *c == delim { 
					state = 1;
					break;
				}
				s.push(*c);
			}
			self.block_offset += l;
			if self.block_offset >= self.block_length {
				let fp = unsafe{&*self.fp};
				self.block_address = fp.offset + (unsafe{fp.begin.offset_from(fp.buffer)} as off_t);
				self.block_offset = 0;
				self.block_length = 0;
			}
			if state != 0 { break }
		}
		if state < 0 {
			if s.is_empty() {
				if state == -1 { Err(Error::new(ErrorKind::UnexpectedEof, "EOF")) }
				else { Err(hts_err("Error uncompressing data".to_string()))	} 
			} else {
				s.clear();
				Err(Error::new(ErrorKind::UnexpectedEof, "Incomplete line"))				
			}
		} else {
			self.uncompressed_address += 1 + s.len() as i64;	
			if s.is_empty() { Ok (0) } // Empty line
			else {
				if delim == b'\n' && s[s.len() - 1] == b'\r' { s.pop(); }			
				Ok(s.len())
			}
		}
	}
	pub fn clear_eof(&mut self) {
		self.set_last_block_eof(0);
		self.set_no_eof_block(0);
		let fp = unsafe{&mut *self.fp};
		fp.set_at_eof(0);
		fp.begin = fp.buffer;
		fp.end = fp.buffer;
	}
}


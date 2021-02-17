use std::io::{self, Write};

pub fn write_u32_slice<W: Write>(w: &mut W, buf: &[u32]) -> io::Result<usize> {
	for x in buf.iter() { w.write_all(&x.to_ne_bytes())? }
	Ok(buf.len())
}

pub fn write_u16_slice<W: Write>(w: &mut W, buf: &[u16]) -> io::Result<usize> {
	for x in buf.iter() { w.write_all(&x.to_ne_bytes())? }
	Ok(buf.len())
}

pub fn write_u64_slice<W: Write>(w: &mut W, buf: &[u64]) -> io::Result<usize> {
	for x in buf.iter() { w.write_all(&x.to_ne_bytes())? }
	Ok(buf.len())
}

pub fn write_f32_slice<W: Write>(w: &mut W, buf: &[f32]) -> io::Result<usize> {
	for x in buf.iter() { w.write_all(&x.to_ne_bytes())? }
	Ok(buf.len())
}

pub fn write_u16<W: Write>(w: &mut W, x: u16) -> io::Result<()> { w.write_all(&x.to_ne_bytes()) }
pub fn write_u32<W: Write>(w: &mut W, x: u32) -> io::Result<()> { w.write_all(&x.to_ne_bytes()) }
pub fn write_u64<W: Write>(w: &mut W, x: u64) -> io::Result<()> { w.write_all(&x.to_ne_bytes()) }
pub fn write_f32<W: Write>(w: &mut W, x: f32) -> io::Result<()> { w.write_all(&x.to_ne_bytes()) }



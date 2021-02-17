use std::io::{self, Write, Seek, BufWriter};
use std::fs::File;
use std::sync::{RwLock, Arc};

use libc::c_int;

pub const ZOOM_LEVELS: usize = 10;

const BB_INITIAL_REDUCTION: usize = 10;
const BW_INITIAL_REDUCTION: usize = 40;
const ZOOM_RES_INCREMENT: usize = 4;

#[derive(Default)]
pub struct ZoomCounts {
	res_end: [u32; ZOOM_LEVELS],
	res_size: [u32; ZOOM_LEVELS],	
}

impl ZoomCounts {
	pub fn add_count(&mut self, pos: u32, zoom_scales: &[u32]) {
		for (i, scale) in zoom_scales.iter().enumerate() {
			if pos >= self.res_end[i] {
				self.res_size[i] += 1;
				self.res_end[i] = pos + scale;
			}
		} 
	}	
	pub fn clear(&mut self) { self.res_end.iter_mut().for_each(|p| *p = 0) }
}

pub fn make_zoom_scales() -> (Vec<u32>, Vec<u32>) {
	let make_scales = |x| {
		let mut v = Vec::with_capacity(ZOOM_LEVELS);
		v.push(x as u32);
		for i in 1..ZOOM_LEVELS { v.push(v[i - 1] * (ZOOM_RES_INCREMENT as u32)) } 
		v
	};

	(make_scales(BB_INITIAL_REDUCTION), make_scales(BW_INITIAL_REDUCTION))
}

// Stored data to allow generation of zoom levels for bigBed and bigWig files
// We store data on two bases in each byte of base_type;
// bits 4-7: base 1, bits 0-3: base 2
//
// bits 0-1: bedmethyl_type
// bit 2: strand (0 == top, 1 == bottom)
// bit 3: non-zero methylation (1 == yes)
// bits 4-5: bedmethyl_type
// bit 6: strand (0 == top, 1 == bottom)
// bit 7: non-zero methylation (1 == yes)
//
// There is one ZoomData structure per contig
//

pub struct ZoomData {
	base_type: Vec<u8>,
	val: Vec<f32>,
} 

impl ZoomData {
	pub fn new(len: usize) -> Self {
		Self {
			base_type: vec!(0; (len + 1) >> 1),
			val: Vec::new()
		}
	}

	pub fn add_data(&mut self, pos: u32, bm_type: u8, strand: char, a: c_int, m: f32) {
		let ix = (pos >> 1) as usize;
		let mut base_type = bm_type;
		if strand == '-' { base_type |= 4 }
		if a > 0 {
			base_type |= 8;
			self.val.push(100.0 * m);
		}
		if (pos & 1) == 0  { base_type <<= 4 }
		self.base_type[ix] |= base_type;	
	}
}


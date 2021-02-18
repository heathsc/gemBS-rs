use std::io::{self, Write, Seek, BufWriter};
use std::fs::File;
use std::sync::{RwLock, Arc};

use libc::c_int;

pub const ZOOM_LEVELS: usize = 10;

const BB_INITIAL_REDUCTION: u32 = 10;
const BW_INITIAL_REDUCTION: u32 = 40;
const ZOOM_RES_INCREMENT: u32 = 4;

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

#[derive(Default)]
pub struct ZoomRec {
	end: u32,       // End base of zoom region (start base is given by the scale)
	count: u32,     // How many data items in this record
	sum_x: f32,
	sum_xsq: f32,
	min: f32,
	max: f32,
}

impl ZoomRec {
	pub fn clear(&mut self, end: u32) {
		self.count = 0;
		self.sum_x = 0.0;
		self.sum_xsq = 0.0;
		self.min = 0.0;
		self.max = 0.0;
		self.end = end;
	}	
	pub fn set(&mut self, end: u32, x: f32) {
		self.count = 1;
		self.sum_x = x;
		self.sum_xsq = x * x;
		self.min = x;
		self.max = x;
		self.end = end;
	}
	pub fn add(&mut self, x: f32) {
		self.count += 1;
		self.sum_x += x;
		self.sum_xsq += x * x;
		self.min = self.min.min(x);
		self.max = self.max.max(x);
	}
	pub fn end(&self) -> u32 { self.end }
	pub fn count(&self) -> u32 { self.count }
	pub fn sum_x(&self) -> f32 { self.sum_x }
	pub fn sum_xsq(&self) -> f32 { self.sum_xsq }
	pub fn min(&self) -> f32 { self.min}
	pub fn max(&self) -> f32 { self.max }
}

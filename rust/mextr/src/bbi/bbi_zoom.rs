use std::io::{self, Write};

use super::bbi_utils::*;

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
	pub fn counts(&self) -> &[u32] { &self.res_size }
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

#[derive(Default, Copy, Clone)]
pub struct ZoomRec {
	id: u32,
	end: u32,       // End base of zoom region (start base is given by the scale)
	count: u32,     // How many data items in this record
	sum_x: f32,
	sum_xsq: f32,
	min: f32,
	max: f32,
}

impl ZoomRec {
	pub fn clear(&mut self) {
		self.count = 0;
		self.sum_x = 0.0;
		self.sum_xsq = 0.0;
		self.min = 0.0;
		self.max = 0.0;
		self.end = 0;
		self.id = 0;
	}	
	pub fn set(&mut self, id: u32, end: u32, x: f32) {
		self.count = 1;
		self.sum_x = x;
		self.sum_xsq = x * x;
		self.min = x;
		self.max = x;
		self.end = end;
		self.id = id;
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
	pub fn id(&self) -> u32 { self.id }
}

#[derive(Default, Copy, Clone)]
pub struct Summary {
	count: u64,     // How many data items in this record
	sum_x: f64,
	sum_xsq: f64,
	min: f64,
	max: f64,
}

impl Summary {
	pub fn add_zrec(&mut self, zr: &ZoomRec) {
		self.count += zr.count as u64;
		self.sum_x += zr.sum_x as f64;
		self.sum_xsq += zr.sum_xsq as f64;
		self.min = self.min.min(zr.min as f64);
		self.max = self.max.max(zr.max as f64);	
	}

	pub fn write<W: Write>(&self, w: &mut W) -> io::Result<usize> {
		write_u64(w, self.count)?;
		write_f64_slice(w, &[self.min, self.max, self.sum_x, self.sum_xsq])
	}
}

pub struct ZoomHeader {
	reduction_level: u32,
	data_offset: u64,
	index_offset: u64,
}

impl ZoomHeader {
	pub fn new(reduction_level: u32, data_offset: u64, index_offset: u64) -> Self {
		Self { reduction_level, data_offset, index_offset }
	}
	
	pub fn write<W: Write>(&self, w: &mut W) -> io::Result<usize> {
		write_u32_slice(w, &[self.reduction_level, 0])?;
		write_u64_slice(w, &[self.data_offset, self.index_offset])
	}
}

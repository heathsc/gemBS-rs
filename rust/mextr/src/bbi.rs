use std::io::{self, Write, Seek};
use std::sync::{RwLock, Arc};
use std::ffi::CString;

use crossbeam_channel::Sender;
use libc::{c_void, memcpy};
use crate::config::{Mode, ConfHash};

pub mod bbi_file_struct;
pub mod compress_bbi;
pub mod write_bbi;
pub mod bbi_zoom;
pub mod bbi_utils;
pub mod tree;
pub mod bbi_finish;

use bbi_zoom::*;
use bbi_file_struct::*;
use bbi_utils::*;

const BB_ITEMS_PER_SLOT: u32 = 512;
const BW_ITEMS_PER_SLOT: u32 = 1024;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BbiBlockType {
	Bb(u8),
	Bw(u8),
}

#[derive(Debug)]
pub struct BbiCtgBlock {
	start: u32,
	end: u32,
	offset: u64,	
}

impl BbiCtgBlock {
	pub fn new(blk: &BbiBlock, offset: u64) -> Self { Self { start: blk.start, end: blk.end, offset }}	
}

#[derive(Debug)]
pub struct BbiBlock {
	start: u32,
	end: u32,
	id: u32,
	idx: u32,
	bbi_type: BbiBlockType,
}

impl BbiBlock {
	pub fn start(&self) -> u32 { self.start }	
	pub fn end(&self) -> u32 { self.end }	
	pub fn id(&self) -> u32 { self.id }	
	pub fn idx(&self) -> u32 { self.idx }	
	pub fn bbi_type(&self) -> BbiBlockType { self.bbi_type }	
}

pub struct BbiBlockBuild {
	block: Option<(BbiBlock, Vec<u8>)>,
	zblock: [Option<(BbiBlock, Vec<u8>)>; ZOOM_LEVELS],
	zidx: [u32; ZOOM_LEVELS], 
	zrec: [ZoomRec; ZOOM_LEVELS],
	summary: Summary,
	bbi_type: BbiBlockType,
	idx: u32,
	n_items: u32,
	n_rec: u64,
	items_per_slot: u32,
	n_zoom_items: [u32; ZOOM_LEVELS],
	n_zoom_rec: [u32; ZOOM_LEVELS],
	zoom_scales: Arc<Vec<u32>>,
	zoom_counts: ZoomCounts,	
}

impl BbiBlockBuild {
	pub fn add_bb_rec(&mut self, id: u32, pos: u32, desc: &str, s: &Sender<BbiMsg>) {
		let v = if let Some((bblk, v)) = self.block.as_mut() { 
			bblk.end = pos + 1;
			v
		} else { 
			self.block = Some((BbiBlock{start: pos, id, end: pos + 1, idx: self.idx, bbi_type: self.bbi_type}, Vec::new())); 
			self.idx += 1;
			&mut self.block.as_mut().unwrap().1
		};	
		let tbuf = [id, pos, pos + 1];
		write_u32_slice(v, &tbuf).expect("Error writing bb data");
		let cdesc = CString::new(desc.as_bytes()).unwrap();		
		v.write_all(cdesc.as_bytes_with_nul()).expect("Error writing bb data");
		self.n_items += 1;
		self.zoom_counts.add_count(pos, &self.zoom_scales);
		if self.n_items >= BB_ITEMS_PER_SLOT { self.finish_bbi(s) }		
	}
	
	pub fn add_bw_rec(&mut self, id: u32, pos: u32, val: f32, s: &Sender<BbiMsg>) {
		let v = if let Some((bblk, v)) = self.block.as_mut() { 
			bblk.end = pos + 1;
			v
		} else { 
			self.block = Some((BbiBlock{start: pos, id, end: pos + 1, idx: self.idx, bbi_type: self.bbi_type}, vec![0u8; 24])); 
			self.idx += 1;
			&mut self.block.as_mut().unwrap().1
		};
		write_u32(v, pos).expect("Error writing BwData");
		write_f32(v, val).expect("Error writing BwData");
		self.n_items += 1;
		self.zoom_counts.add_count(pos, &self.zoom_scales);
		if self.n_items >= BW_ITEMS_PER_SLOT { self.finish_bbi(s) }			
	}
	
	pub fn add_zoom_obs(&mut self, id: u32, pos: u32, val: f32, s: &Sender<BbiMsg>) {
		for (i, zr) in self.zrec.iter_mut().enumerate() {
			if pos >= zr.end() {
				if zr.count() > 0 {
					self.n_zoom_rec[i] += 1;
					let scale = self.zoom_scales[i];
					let start = zr.end() - scale;
					let v = if let Some((blk, v)) = self.zblock[i].as_mut() {
						blk.end = zr.end();
						v
					} else {				
						self.zblock[i] = Some((BbiBlock{start, id, end: zr.end(), idx: self.zidx[i], bbi_type: self.bbi_type}, Vec::new())); 
						self.zidx[i] += 1;
						&mut self.zblock[i].as_mut().unwrap().1
					};
					let tbuf = [id, start, zr.end(), zr.count()];
					write_u32_slice(v, &tbuf).expect("Error writing zoom data");
					let tbuf = [zr.min(), zr.max(), zr.sum_x(), zr.sum_xsq()];
					write_f32_slice(v, &tbuf).expect("Error writing zoom data");
					self.n_zoom_items[i] += 1;
					if i == 0 { self.summary.add_zrec(&zr) }
					if self.n_zoom_items[i] >= self.items_per_slot {
						let (blk, mut v) = self.zblock[i].take().unwrap();
						v.shrink_to_fit();
						s.send(BbiMsg::ZData((blk, v, i as u32))).expect("Error sending Zoom data block");
						self.n_zoom_items[i] = 0;
					}
				}
				zr.set(id, pos + self.zoom_scales[i], val);
			} else { zr.add(val) }
		}
	}
	
	pub fn flush_zoom_obs(&mut self) {
		for (i, zr) in self.zrec.iter_mut().enumerate() {
			if zr.count() > 0 {
				let scale = self.zoom_scales[i];
				let start = zr.end() - scale;
				let v = if let Some((blk, v)) = self.zblock[i].as_mut() {
					blk.end = zr.end();
					v
				} else {				
					self.zblock[i] = Some((BbiBlock{start, id: zr.id(), end: zr.end(), idx: self.zidx[i], bbi_type: self.bbi_type}, Vec::new())); 
					self.zidx[i] += 1;
					&mut self.zblock[i].as_mut().unwrap().1
				};
				let tbuf = [zr.id(), start, zr.end(), zr.count()];
				write_u32_slice(v, &tbuf).expect("Error writing zoom data");
				let tbuf = [zr.min(), zr.max(), zr.sum_x(), zr.sum_xsq()];
				write_f32_slice(v, &tbuf).expect("Error writing zoom data");
				if i == 0 { self.summary.add_zrec(&zr) }
				self.n_zoom_items[i] += 1;
			}	
			zr.clear();			
		}
	}
	
	pub fn finish_bbi(&mut self, s: &Sender<BbiMsg>) {
		let (blk, mut buf) = self.block.take().unwrap();
		if matches!(self.bbi_type, BbiBlockType::Bb(_)) { self.n_rec += self.n_items as u64 } 
		else { 
			let bw_hdr = BwDataHeader::init(&blk, self.n_items as u16);
			unsafe { memcpy(buf.as_mut_ptr() as *mut c_void, &bw_hdr as *const BwDataHeader as *const c_void, 24); }
			self.n_rec += 1; 
		}
		buf.shrink_to_fit();
		s.send(BbiMsg::Data((blk, buf))).expect("Error sending BbiBlock");
		self.n_items = 0;			
	}

	pub fn finish(&mut self, s: &Sender<BbiMsg>) {
		if self.n_items > 0 { self.finish_bbi(s) }
		self.flush_zoom_obs();
		for (i, j) in self.n_zoom_items.iter_mut().enumerate().filter(|(_, j)| **j > 0) {
			let (blk, mut v) = self.zblock[i].take().unwrap();
			v.shrink_to_fit();
			s.send(BbiMsg::ZData((blk, v, i as u32))).expect("Error sending Zoom data block");
			*j = 0;
		}
	}

	pub fn end_of_input(&mut self, s: &Sender<BbiMsg>) { 
		s.send(BbiMsg::EndOfSection((self.bbi_type, self.idx, self.zidx))).expect("Error sending BbiBlock") 
	}	
	pub fn clear_counts(&mut self) { 
		self.zoom_counts.clear();
		for zr in self.zrec.iter_mut() { zr.clear() }
	}
	pub fn n_rec(&self) -> u64 { self.n_rec }
	pub fn n_zoom_rec(&self) -> [u32; ZOOM_LEVELS] { self.n_zoom_rec }
	pub fn zoom_scales(&self) -> Arc<Vec<u32>> { self.zoom_scales.clone() }
	pub fn summary(&self) -> Summary { self.summary }
}

pub struct BbiFile {
	name: String,
	build: RwLock<BbiBlockBuild>,
}

impl BbiFile {
	pub fn new<S: AsRef<str>>(name: S, ix: usize, zoom_scales: Arc<Vec<u32>>, bb_flag: bool) -> io::Result<Self> {
		let name = name.as_ref().to_owned();
		let (bbi_type, items_per_slot) = if bb_flag { (BbiBlockType::Bb(ix as u8), BB_ITEMS_PER_SLOT) } else { (BbiBlockType::Bw(ix as u8), BW_ITEMS_PER_SLOT) };
		let build = RwLock::new(BbiBlockBuild{ block: None, n_rec: 0, idx: 0, n_items: 0, bbi_type, zoom_scales, 
			items_per_slot, n_zoom_items: Default::default(), n_zoom_rec: Default::default(),
			zidx: Default::default(), zblock: Default::default(), zrec: Default::default(), summary: Default::default(), zoom_counts: Default::default()});
		Ok(Self{name, build } )
	}
	pub fn build(&self) -> &RwLock<BbiBlockBuild> { &self.build }	
	pub fn name(&self) -> &str { &self.name }
}

pub enum BbiMsg {
	Data((BbiBlock, Vec<u8>)),
	ZData((BbiBlock, Vec<u8>, u32)),
	EndOfSection((BbiBlockType, u32, [u32; ZOOM_LEVELS])),	
}

pub struct Bbi {
	bb_files: Vec<BbiFile>,
	bw_files: Vec<BbiFile>,
	n_output_ctgs: usize,
	sender: Option<Sender<BbiMsg>>,
}

impl Bbi {
	pub fn init<S: AsRef<str>>(prefix: S, sender: Sender<BbiMsg>, chash: &ConfHash) -> io::Result<Self> {
		let strand_specific = matches!(chash.get_mode("bw_mode"), Mode::StrandSpecific);	
		let (bb_zoom_scales, bw_zoom_scales) = {
			let (v1, v2) = make_zoom_scales();
			(Arc::new(v1), Arc::new(v2))
		};
		
		let n_output_ctgs = chash.vcf_contigs().iter().filter(|x| x.out_ix().is_some()).count();
		
		let bb_files = vec!(
			BbiFile::new(format!("{}_cpg.bb", prefix.as_ref()), 0, bb_zoom_scales.clone(), true)?,
			BbiFile::new(format!("{}_chg.bb", prefix.as_ref()), 1, bb_zoom_scales.clone(), true)?,
			BbiFile::new(format!("{}_chh.bb", prefix.as_ref()), 2, bb_zoom_scales, true)?
		);
		let bw_files = if strand_specific { vec!( 
			BbiFile::new(format!("{}_pos.bw", prefix.as_ref()), 0, bw_zoom_scales.clone(), false)?,
			BbiFile::new(format!("{}_neg.bw", prefix.as_ref()), 1, bw_zoom_scales, false)?
		)} else { vec!(BbiFile::new(format!("{}.bw", prefix.as_ref()), 0, bw_zoom_scales, false)?)};
		
		Ok(Bbi{bb_files, bw_files, sender: Some(sender), n_output_ctgs})

	}
	pub fn drop_sender(&mut self) { 
		self.sender = None;
		trace!("Bbi drop_sender()");
	}
	pub fn n_rec(&self, bbi_type: BbiBlockType) -> u64 {
		trace!("Bbi nrec()");
		let n = match bbi_type {
			BbiBlockType::Bb(i) => self.bb_files[i as usize].build.read().unwrap().n_rec,
			BbiBlockType::Bw(i) => self.bw_files[i as usize].build.read().unwrap().n_rec,
		};
		trace!("Bbi nrec() done");
		n
	}
	pub fn bb_files(&self) -> &[BbiFile] { &self.bb_files }
	pub fn bw_files(&self) -> &[BbiFile] { &self.bw_files }
	pub fn n_output_ctgs(&self) -> usize { self.n_output_ctgs }
	pub fn sender(&self) -> Option<&Sender<BbiMsg>> { self.sender.as_ref() }
	pub fn finish(&self) {
		let sender = self.sender.as_ref().expect("Sender is not set");
		trace!("Bbi finish()");
		for mut bb in self.bb_files.iter().map(|f| f.build().write().unwrap()) {
			bb.finish(sender);
			bb.end_of_input(sender);
		}
		for mut bw in self.bw_files.iter().map(|f| f.build().write().unwrap()) {
			bw.finish(sender);
			bw.end_of_input(sender);
		}
		trace!("Bbi finish() done");
	}
}

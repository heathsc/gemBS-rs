use std::io::{self, Write, Seek, BufWriter};
use std::fs::File;
use std::sync::{RwLock, Arc};

use crossbeam_channel::Sender;
use libc::{c_void, size_t, memcpy};
use crate::config::{Mode, ConfHash};

pub mod bbi_file_struct;
pub mod compress_bbi;
pub mod print_bbi;
pub mod bbi_zoom;
pub mod bbi_utils;
pub mod tree;

use bbi_zoom::*;
use bbi_file_struct::*;
use bbi_utils::*;

const BB_ITEMS_PER_SLOT: u32 = 512;
const BW_ITEMS_PER_SLOT: u32 = 1024;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum BbiBlockType {
	Bb(u8),
	Bw(u8),
}

pub struct BbiCtgBlock {
	start: u32,
	end: u32,
	offset: u64,	
}

impl BbiCtgBlock {
	pub fn new(blk: &BbiBlock, offset: u64) -> Self { Self { start: blk.start, end: blk.end, offset }}	
}

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
	bbi_type: BbiBlockType,
	idx: u32,
	n_items: u32,
	n_rec: u32,
	zoom_scales: Arc<Vec<u32>>,
	zoom_counts: ZoomCounts,	
}

impl BbiBlockBuild {
	pub fn add_bb_rec(&mut self, id: u32, pos: u32, desc: &str, s: &Sender<(BbiBlock, Vec<u8>)>) {
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
		v.write_all(desc.as_bytes()).expect("Error writing bb data");
		self.n_items += 1;
		self.zoom_counts.add_count(pos, &self.zoom_scales);
		if self.n_items >= BB_ITEMS_PER_SLOT { self.finish(s) }		
	}
	
	pub fn add_bw_rec(&mut self, id: u32, pos: u32, val: f32, s: &Sender<(BbiBlock, Vec<u8>)>) {
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
		if self.n_items >= BW_ITEMS_PER_SLOT { self.finish(s) }			
	}
	
	pub fn finish(&mut self, s: &Sender<(BbiBlock, Vec<u8>)>) {
		if self.n_items > 0 {
			let (blk, mut buf) = self.block.take().unwrap();
			if matches!(self.bbi_type, BbiBlockType::Bb(_)) { self.n_rec += self.n_items } 
			else { 
				let bw_hdr = BwDataHeader::init(&blk, self.n_items as u16);
				unsafe {
					memcpy(buf.as_mut_ptr() as *mut c_void, &bw_hdr as *const BwDataHeader as *const c_void, 24);
				}
				self.n_rec += 1; 
			}
			s.send((blk, buf)).expect("Error sending BbiBlock");
			self.n_items = 0;			
		}
	}
	
	pub fn clear_counts(&mut self) { self.zoom_counts.clear() }
}

pub struct BbiFile {
	name: String,
	build: RwLock<BbiBlockBuild>,
	ctg_blocks: Arc<RwLock<Vec<Vec<BbiCtgBlock>>>>,	
}

impl BbiFile {
	pub fn new<S: AsRef<str>>(name: S, ix: usize, zoom_scales: Arc<Vec<u32>>, n_ctgs: usize, bb_flag: bool) -> io::Result<Self> {
		let name = name.as_ref().to_owned();
		let bbi_type = if bb_flag { BbiBlockType::Bb(ix as u8) } else { BbiBlockType::Bw(ix as u8) };
		let build = RwLock::new(BbiBlockBuild{ block: None, n_rec: 0, idx: 0, n_items: 0, bbi_type, zoom_scales, zoom_counts: Default::default()});
		let blocks: Vec<Vec<BbiCtgBlock>> = (0..n_ctgs).map(|_| Vec::new()).collect();
		Ok(Self{name, build, ctg_blocks: Arc::new(RwLock::new(blocks))} )
	}
	pub fn build(&self) -> &RwLock<BbiBlockBuild> { &self.build }	
	pub fn name(&self) -> &str { &self.name }
	pub fn ctg_blocks(&self) -> Arc<RwLock<Vec<Vec<BbiCtgBlock>>>> { self.ctg_blocks.clone() }
}

pub struct Bbi {
	bb_files: Vec<BbiFile>,
	bw_files: Vec<BbiFile>,
	n_output_ctgs: usize,
	sender: Option<Sender<(BbiBlock, Vec<u8>)>>,
	bb_zoom_scales: Arc<Vec<u32>>,
	bw_zoom_scales: Arc<Vec<u32>>,
}

impl Bbi {
	pub fn init<S: AsRef<str>>(prefix: S, sender: Sender<(BbiBlock, Vec<u8>)>, chash: &ConfHash) -> io::Result<Self> {
		let strand_specific = matches!(chash.get_mode("bw_mode"), Mode::StrandSpecific);	
		let (bb_zoom_scales, bw_zoom_scales) = {
			let (v1, v2) = make_zoom_scales();
			(Arc::new(v1), Arc::new(v2))
		};
		
		let n_output_ctgs = chash.vcf_contigs().iter().filter(|x| x.out_ix().is_some()).count();
		
		let bb_files = vec!(
			BbiFile::new(format!("{}_cpg.bb", prefix.as_ref()), 0, bb_zoom_scales.clone(), n_output_ctgs, true)?,
			BbiFile::new(format!("{}_chg.bb", prefix.as_ref()), 1, bb_zoom_scales.clone(), n_output_ctgs, true)?,
			BbiFile::new(format!("{}_chh.bb", prefix.as_ref()), 2, bb_zoom_scales.clone(), n_output_ctgs, true)?
		);
		let bw_files = if strand_specific { vec!( 
			BbiFile::new(format!("{}_pos.bw", prefix.as_ref()), 0, bw_zoom_scales.clone(), n_output_ctgs, false)?,
			BbiFile::new(format!("{}_neg.bw", prefix.as_ref()), 1, bw_zoom_scales.clone(), n_output_ctgs, false)?
		)} else { vec!(BbiFile::new(format!("{}.bw", prefix.as_ref()), 0, bw_zoom_scales.clone(), n_output_ctgs, false)?)};
		
		Ok(Bbi{bb_files, bw_files, sender: Some(sender), bb_zoom_scales, bw_zoom_scales, n_output_ctgs})

	}
	pub fn drop_sender(&mut self) { self.sender = None }
	pub fn bb_files(&self) -> &[BbiFile] { &self.bb_files }
	pub fn bw_files(&self) -> &[BbiFile] { &self.bw_files }
	pub fn sender(&self) -> Option<&Sender<(BbiBlock, Vec<u8>)>> { self.sender.as_ref() }
	pub fn finish(&self) {
		let sender = self.sender.as_ref().expect("Sender is not set");
		for mut bb in self.bb_files.iter().map(|f| f.build().write().unwrap()) {
			bb.finish(sender);
		}
		for mut bw in self.bw_files.iter().map(|f| f.build().write().unwrap()) {
			bw.finish(sender);
		}
	}
}

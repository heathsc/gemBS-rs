use std::sync::Arc;
use std::fs::File;
use std::io::{BufWriter, Write, SeekFrom};
use std::collections::HashMap;
use std::thread;

use crossbeam_channel::Receiver;

use crate::config::ConfHash;
use super::*;
use super::tree::CtgTree;
use super::bbi_finish::bbi_finish;

pub struct BbiWriter {
	pub fp: BufWriter<File>,
	pub ctg_blocks: Vec<Vec<BbiCtgBlock>>,	
	pub zoom_data: [Vec<(BbiBlock, Vec<u8>)>; ZOOM_LEVELS],
	pub header: BbiHeader,
	pub name: String,
	bbi_type: BbiBlockType,
	n_ctgs: usize,
	pub index_offset: u64,
}

impl BbiWriter {
	pub fn fp(&mut self) -> &mut BufWriter<File> { &mut self.fp }
	pub fn header(&mut self) -> &mut BbiHeader { &mut self.header }
	pub fn init(bbi_file: &BbiFile, bbi_type: BbiBlockType, n_ctgs: usize) -> Self {
		trace!("In init for {:?}", bbi_type);
		let bb_flag = matches!(bbi_type, BbiBlockType::Bb(_));
		let header = BbiHeader::new(bb_flag);
		let name = bbi_file.name().to_owned();
		let fp = match File::create(&name) {
			Ok(f) => BufWriter::new(f),
			Err(e) => panic!("Could not open output file {}: {}", &name, e),
		};
		let blocks: Vec<Vec<BbiCtgBlock>> = (0..n_ctgs).map(|_| Vec::new()).collect();
		trace!("Leaving init for {:?}", bbi_type);
		Self{header, ctg_blocks: blocks, n_ctgs, name, bbi_type, zoom_data: Default::default(), fp, index_offset: 0}
	}	
	pub fn bbi_type(&self) -> BbiBlockType { self.bbi_type }
	pub fn ctg_blocks(&self) -> &[Vec<BbiCtgBlock>] { &self.ctg_blocks }
	pub fn index_offset(&self) -> u64 { self.index_offset }
	pub fn clear_ctg_blocks(&mut self) {
		let blocks: Vec<Vec<BbiCtgBlock>> = (0..self.n_ctgs).map(|_| Vec::new()).collect();
		self.ctg_blocks = blocks;
	}
}

fn init_writers(ch: &ConfHash) -> (Vec<BbiWriter>, Vec<BbiWriter>) {
	trace!("write_bbi_thread - initialize writers");
	let bbi_ref = ch.bbi().read().unwrap();
	let bbi = bbi_ref.as_ref().expect("Bbi not set");
	let n_ctgs = bbi.n_output_ctgs();
	trace!("init bb writers");
	let mut bb_writers: Vec<_> = bbi.bb_files().iter().enumerate().map(|(i, f)| BbiWriter::init(f, BbiBlockType::Bb(i as u8), n_ctgs)).collect();
	trace!("init bw writers");
	let mut bw_writers: Vec<_> = bbi.bw_files().iter().enumerate().map(|(i, f)| BbiWriter::init(f, BbiBlockType::Bw(i as u8), n_ctgs)).collect();
	let ctg_tree = CtgTree::init(ch);
	for w in bb_writers.iter_mut().chain(bw_writers.iter_mut()) { 
		ctg_tree.write(w).expect("Error writing header to bbi file"); 
	}
	trace!("initalize writers finished");
	(bb_writers, bw_writers)
}

enum State {
	Writing,
	Finishing(u32, [u32; ZOOM_LEVELS]),
}

struct WriterState<'a> {
	writer: &'a mut BbiWriter,
	curr_idx: u32,
	curr_zoom_idx: [u32; ZOOM_LEVELS],
	state: State,
	store: HashMap<u32, (BbiBlock, Vec<u8>)>,
	zstore: HashMap<(u32, u32), (BbiBlock, Vec<u8>)>,
} 

impl <'a>WriterState<'a> {
	fn new(writer: &'a mut BbiWriter) -> WriterState<'a> {
		Self { writer, curr_idx: 0, curr_zoom_idx: Default::default(), 
			state: State::Writing, store: HashMap::new(), zstore: HashMap::new() }
	}	
	fn clear_state(&mut self) {
		assert!(self.store.is_empty());
		self.curr_idx = 0;
		self.state = State::Writing;
		debug!("clear_state() called for {:?}", self.writer.bbi_type);
	}
	fn check_idx(&self, idx: u32, zoom_idx: &[u32]) -> bool {
		assert!(idx >= self.curr_idx);
		if idx == self.curr_idx {
			for (i, j) in self.curr_zoom_idx.iter().enumerate() {
				assert!(zoom_idx[i] >= *j);
				if zoom_idx[i] != *j { return false }
			}
			true
		} else { false }
	}
}

fn add_block(ws: &mut WriterState, blk: &BbiBlock) {
	let pos = ws.writer.fp.seek(SeekFrom::Current(0)).expect("Error getting position from BBI file");
	let ctg_block = &mut ws.writer.ctg_blocks;
	let blocks = &mut ctg_block[blk.id() as usize];
	blocks.push(BbiCtgBlock::new(blk, pos));
}

pub fn write_bbi_thread(ch: Arc<ConfHash>, r: Receiver<BbiMsg>) {
	info!("write_bbi_thread starting up");
	let (mut bb_writers, mut bw_writers) = init_writers(&ch);
	let mut state = HashMap::new();
	
	for (ix, w) in bb_writers.iter_mut().enumerate() { state.insert(BbiBlockType::Bb(ix as u8), WriterState::new(w)); }
	for (ix, w) in bw_writers.iter_mut().enumerate() { state.insert(BbiBlockType::Bw(ix as u8), WriterState::new(w)); }
	
	for msg in r.iter() {
		match msg {
			BbiMsg::Data((blk, v)) => {
				let ws = state.get_mut(&blk.bbi_type()).expect("Unexpected block type");
				if blk.idx() == ws.curr_idx {
					add_block(ws, &blk);
					ws.writer.fp.write_all(&v).expect("Couldn't write out compressed block");
					drop(v);
					ws.curr_idx += 1;
					while let Some((blk1, v1)) = ws.store.remove(&ws.curr_idx) {
						add_block(ws, &blk1);
						ws.writer.fp.write_all(&v1).expect("Couldn't write out compressed block");
						ws.curr_idx += 1;
					}
				} else {
					assert!(blk.idx() > ws.curr_idx); 
					ws.store.insert(blk.idx(), (blk, v)); 
				}	
				if let State::Finishing(ix, v) = ws.state {
					if ws.check_idx(ix, &v) { ws.clear_state() }
				}	
			},
			BbiMsg::ZData((blk, v, level)) => {
				let ws = state.get_mut(&blk.bbi_type()).expect("Unexpected block type");
				let l = level as usize;
				if blk.idx() == ws.curr_zoom_idx[l] {
					ws.writer.zoom_data[level as usize].push((blk, v));
					ws.curr_zoom_idx[l] += 1;
					while let Some((blk1, v1)) = ws.zstore.remove(&(ws.curr_zoom_idx[l], level)) {
						ws.writer.zoom_data[level as usize].push((blk1, v1));
						ws.curr_zoom_idx[l] += 1;
					}
				} else {
					assert!(blk.idx() > ws.curr_zoom_idx[l]); 
					ws.zstore.insert((blk.idx(), level), (blk, v)); 
				}	
				if let State::Finishing(ix, v) = ws.state {
					if ws.check_idx(ix, &v) { ws.clear_state() }
				}	
				
			},
			BbiMsg::EndOfSection((bbi_type, ix, v)) => {
				let ws = state.get_mut(&bbi_type).expect("Unexpected block type");
				debug!("writer for bbi_type: {:?} got end of section with index {} {:?} (curr_index: {}, {:?})", bbi_type, ix, v, ws.curr_idx, ws.curr_zoom_idx);			
				if ws.check_idx(ix, &v) { 
					ws.clear_state() 
				} else {
					ws.state = State::Finishing(ix, v);
					debug!("writer for bbi_type: {:?} waiting for index {} (curr_index: {}, {:?})", bbi_type, ix, ws.curr_idx, ws.curr_zoom_idx);			
				}
			},
		}
	}	
	
	// Debugging
	for (bbi_type, ws) in state.iter() {
		debug!("Writer state: {:?} {} {:?}", bbi_type, ws.curr_idx, ws.curr_zoom_idx);
		assert!(ws.store.is_empty());
		assert!(ws.zstore.is_empty());
	}
	
	// Store start of index in file
	for w in bb_writers.iter_mut().chain(bw_writers.iter_mut()) { w.index_offset = w.fp.seek(SeekFrom::Current(0)).expect("Error getting position from BBI file") }
	
	// Launch threads to generate indices and write out zoom data
	let mut threads = Vec::with_capacity(5);
	for w in bb_writers.drain(..).chain(bw_writers.drain(..)) { 
		let chc = ch.clone();
		let th = thread::spawn(move || bbi_finish(chc, w));
		threads.push(th);
	}	
	
	// Wait for daughter threads
	for th in threads.drain(..) { th.join().unwrap() }
	
	info!("write_bbi_thread shutting down");
	
}

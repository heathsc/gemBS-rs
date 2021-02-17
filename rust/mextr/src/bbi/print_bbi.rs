use std::sync::Arc;
use std::fs::File;
use std::io::{BufWriter, Write, SeekFrom};
use std::collections::HashMap;

use crossbeam_channel::Receiver;

use crate::config::ConfHash;
use super::*;
use super::tree::CtgTree;

pub struct BbiWriter {
	fp: BufWriter<File>,
	ctg_blocks: Arc<RwLock<Vec<Vec<BbiCtgBlock>>>>,	
	header: BbiHeader,
	index_offset: u64,
}

impl BbiWriter {
	pub fn fp(&mut self) -> &mut BufWriter<File> { &mut self.fp }
	pub fn header(&mut self) -> &mut BbiHeader { &mut self.header }
	pub fn init(bbi_file: &BbiFile, bb_flag: bool) -> Self {
		let header = BbiHeader::new(bb_flag);
		let fp = match File::create(bbi_file.name()) {
			Ok(f) => BufWriter::new(f),
			Err(e) => panic!("Could not open output file {}: {}", bbi_file.name(), e),
		};
		Self{header, ctg_blocks: bbi_file.ctg_blocks(), fp, index_offset: 0}
	}	
}

fn init_writers(ch: &ConfHash) -> (Vec<BbiWriter>, Vec<BbiWriter>) {
	let bbi_ref = ch.bbi().read().unwrap();
	let bbi = bbi_ref.as_ref().expect("Bbi not set");
	let mut bb_writers: Vec<_> = bbi.bb_files().iter().map(|f| BbiWriter::init(f, true)).collect();
	let mut bw_writers: Vec<_> = bbi.bw_files().iter().map(|f| BbiWriter::init(f, false)).collect();
	let ctg_tree = CtgTree::init(ch);
	for w in bb_writers.iter_mut().chain(bw_writers.iter_mut()) { 
		ctg_tree.write(w).expect("Error writing header to bbi file"); 
	}
	(bb_writers, bw_writers)
}

struct WriterState<'a> {
	writer: &'a mut BbiWriter,
	curr_idx: u32,
	store: HashMap<u32, (BbiBlock, Vec<u8>)>,
} 

impl <'a>WriterState<'a> {
	fn new(writer: &'a mut BbiWriter) -> WriterState<'a> {
		Self { writer, curr_idx: 0, store: HashMap::new() }
	}	
}

fn add_block(ws: &mut WriterState, blk: &BbiBlock) {
	let pos = ws.writer.fp.seek(SeekFrom::Current(0)).expect("Error getting position from BBI file");
	let mut ctg_block = ws.writer.ctg_blocks.write().unwrap();
	let blocks = &mut ctg_block[blk.id() as usize];
	blocks.push(BbiCtgBlock::new(blk, pos));
}

pub fn print_bbi_thread(ch: Arc<ConfHash>, r: Receiver<(BbiBlock, Vec<u8>)>) {
	info!("print_bbi_thread starting up");

	let (mut bb_writers, mut bw_writers) = init_writers(&ch);
	let mut state = HashMap::new();
	
	for (ix, w) in bb_writers.iter_mut().enumerate() { state.insert(BbiBlockType::Bb(ix as u8), WriterState::new(w)); }
	for (ix, w) in bw_writers.iter_mut().enumerate() { state.insert(BbiBlockType::Bw(ix as u8), WriterState::new(w)); }
	
	for (blk, v) in r.iter() {
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
		} else { ws.store.insert(blk.idx(), (blk, v)); }
	}	
	// Store start of index in file
	for w in bb_writers.iter_mut().chain(bw_writers.iter_mut()) { w.index_offset = w.fp.seek(SeekFrom::Current(0)).expect("Error getting position from BBI file") }
	
	info!("print_bbi_thread shutting down");
	
}

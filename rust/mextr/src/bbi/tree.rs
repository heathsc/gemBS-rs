use std::io::{self, Seek, SeekFrom, BufWriter, Write};
use std::fs::File;
use std::convert::TryInto;

use crate::config::{VcfContig, ConfHash};
use super::write_bbi::BbiWriter;
use super::bbi_utils::*;
use super::*;

const BLOCK_SIZE: usize = 256;

pub struct CtgTree<'a> {
	n_nodes: usize,
	ctgs: Vec<&'a VcfContig>,
	key_len: u32,
	block_size: u32,
	width: Vec<u32>,
	start_ix: Vec<Vec<u32>>,
}

impl <'a>CtgTree<'a> {
	pub fn init(ch: &'a ConfHash) -> CtgTree<'a> {
		// Collect and sort contigs use for output
		let mut ctgs: Vec<_> = ch.vcf_contigs().iter().filter(|x| x.out_ix().is_some()).collect();
		ctgs.sort_by_key(|x| x.out_ix().unwrap());
		let key_len = ctgs.iter().map(|x| x.name().len()).max().unwrap() as u32;
		let n_nodes = ctgs.len();
		assert!(n_nodes > 0);
		// Set widths
		let mut width = Vec::new();
		let mut n1 = n_nodes;
		loop {
			n1 = (n1 + BLOCK_SIZE - 1) / BLOCK_SIZE;
			width.push(n1 as u32);
			if n1 <= 1 { break }			
		} 
		let depth = width.len();
		let block_size = if depth > 1 { BLOCK_SIZE as u32 } else { n_nodes as u32 };
		let mut start_ix = Vec::with_capacity(depth);
		for(level, w) in width.iter().map(|x| *x as isize).enumerate() {
			let mut start = Vec::with_capacity((w + 1) as usize);
			
			// For higher level nodes we balance node sizes across the tree, but for the level 1 nodes
			// (one above the leaves) they all have to have the same number of entries (block_size) apart
			// from the last one which normally has less.  This is to allow the reader to quickly go from
			// chromosome ID to the key
			let w1 = if level > 0 {
				let w1 = width[level - 1] as isize;
				let k = w1 / w;
				let o1 = w * k - w1;
				let o2 = w * (k + 1) - w1;
				let mut d = 0;
				let mut off = 0;
				for _ in 0..w {
					start.push(off as u32);
					off += if (d + o1).abs() < (d + o2).abs() {
						d += o1;
						k
					} else {
						d += o2;
						k + 1
					}
				}
				w1 as u32
			} else { 
				for i in 0..w as u32 { start.push(i * block_size)}
				n_nodes as u32
			};
			start.push(w1);
			start_ix.push(start);
		}
		CtgTree{n_nodes, ctgs, key_len, width, block_size, start_ix}	
	}
	fn depth(&self) -> usize { self.width.len() }	
	
	pub fn write(&self, wrt: &mut BbiWriter) -> io::Result<()> {	
		let offset = wrt.header().chromosome_tree_offset();
		let w = wrt.fp();
		w.seek(SeekFrom::Start(offset))?;
		let tmp: [u32; 4] = [ 0x78CA8C91, self.block_size, self.key_len, 8];
		write_u32_slice(w, &tmp)?;
		let tmp: [u64; 2] = [ self.n_nodes as u64, 0 ];
		write_u64_slice(w, &tmp)?;
		let depth = self.depth();
		for i in 0..depth - 1 { self.write_non_leaf_level(w, depth - 1 - i)? }
		self.write_leaf_level(w)?;
		let pos = w.seek(SeekFrom::Current(4))?;
		wrt.header().set_full_data_offset(pos - 4);
		Ok(())
	}

	fn ctg_id_lookup(&self, i: usize, level: usize) -> usize {
		let j = self.start_ix[level][i] as usize;
		if level > 0 { self.ctg_id_lookup(j, level - 1) } else { j }	
	}
	
	// Write contig name padded with zeroes if necessary to arrive at size of zeroes slice
	fn write_ctg_name(&self, w: &mut BufWriter<File>, i: usize, zeroes: &[u8]) -> io::Result<()> {
		let ctg_name = self.ctgs[i].name();
		w.write_all(ctg_name.as_bytes())?;
		let name_len = ctg_name.len();
		if name_len < self.key_len as usize { w.write_all(&zeroes[name_len..])? }
		Ok(())
	}
	
	fn write_non_leaf_level(&self, w: &mut BufWriter<File>, level: usize) -> io::Result<()> {
		assert!(level > 0);
		let pos = w.seek(SeekFrom::Current(0))? as usize;
		let zeroes = vec![0u8; self.key_len as usize];
		let n = self.width[level] as usize; // Nodes at this level
		let n1 = self.width[level - 1] as usize; // Nodes at next (lower) level
		let item_size = (self.key_len + 8) as usize;
		let mut off = pos + 4 * n + item_size * n1; // Offset of first node at next (lower) level
		let start = &self.start_ix[level];
		for i in 0..n {
			let a = start[i] as usize;
			let b = start[i + 1] as usize;
			let n_items = b - a;
			let tmp: [u16; 2] = [0, n_items.try_into().unwrap()];
			write_u16_slice(w, &tmp)?;
			for j in a..b {
				self.write_ctg_name(w, self.ctg_id_lookup(j, level - 1), &zeroes)?;
				// Write child node offset
				write_u64(w, off as u64)?;
				off += 4 + item_size * (start[j + 1] - start[j]) as usize;
			} 
		}
		Ok(())
	}
	
	fn write_leaf_level(&self, w: &mut BufWriter<File>) -> io::Result<()> {
		let zeroes = vec![0u8; self.key_len as usize];
		let n = self.width[0] as usize; // Nodes at this level
		let start = &self.start_ix[0];
		for i in 0..n {
			let a = start[i] as usize;
			let b = start[i + 1] as usize;
			let n_items = b - a;
			w.write_all(&[1, 0])?; // Node type leaf
			write_u16(w, n_items.try_into().unwrap())?;
			for j in a..b {
				self.write_ctg_name(w, j, &zeroes)?;
				let tmp: [u32; 2] = [j as u32, self.ctgs[j].length() as u32];
				write_u32_slice(w, &tmp)?;
			}
						
		}
		Ok(())
	}
}

struct RNode {
	start_ctg: u32,
	end_ctg: u32,
	start_base: u32,
	end_base: u32,
	start_idx: u32,
}

pub struct RTree<'a> {
	n_nodes: usize,
	n_items: u32,
	offset: u64,
	ctg_blocks: &'a [Vec<BbiCtgBlock>],
	ctg_blk_ends: Vec<usize>,
	block_size: u32,
	width: Vec<u32>,
	start: Vec<Vec<RNode>>,
}

impl <'a>RTree<'a> {
	pub fn init(ctg_blocks: &'a [Vec<BbiCtgBlock>], n_items: u32, offset: u64) -> Self {
		// Store 1 after the last block for each ctg
		let mut ctg_blk_ends = Vec::with_capacity(ctg_blocks.len());
		ctg_blk_ends.push(ctg_blocks[0].len());
		for (i, v) in ctg_blocks[1..].iter().enumerate() { ctg_blk_ends.push(ctg_blk_ends[i] + v.len())}
		let n_nodes = *ctg_blk_ends.last().unwrap();

		// Set widths
		let mut width = Vec::new();
		let mut n1 = n_nodes;
		loop {
			n1 = (n1 + BLOCK_SIZE - 1) / BLOCK_SIZE;
			width.push(n1 as u32);
			if n1 <= 1 { break }			
		} 
		let depth = width.len();
		let block_size = if depth > 1 { BLOCK_SIZE as u32 } else { n_nodes as u32 };
		let mut start: Vec<Vec<RNode>> = Vec::with_capacity(depth);
		for(level, w) in width.iter().map(|x| *x as isize).enumerate() {
			let mut nodes: Vec<RNode> = Vec::with_capacity((w + 1) as usize);
			// For higher level nodes we balance node sizes across the tree, but for the level 1 nodes
			// (one above the leaves) they all have to have the same number of entries (block_size) apart
			// from the last one which normally has less.  This is to allow the reader to quickly go from
			// chromosome ID to the key
			let w1 = if level > 0 { width[level -1] as isize } else { n_nodes as isize};
			let k = w1 / w;
			let o1 = w * k - w1;
			let o2 = w * (k + 1) - w1;
			let mut d = 0;
			let mut off = 0;
			for _ in 0..w {
				let sz = if (d + o1).abs() < (d + o2).abs() {
					d += o1;
					k as usize
				} else {
					d += o2;
					(k + 1) as usize
				};
				let node = if level > 0 {
					let rn1 = &start[level - 1];
					RNode {
						start_ctg: rn1[off].start_ctg,
						start_base: rn1[off].start_base,
						end_ctg: rn1[off + sz - 1].end_ctg,
						end_base: rn1[off + sz - 1].end_base,
						start_idx: off as u32
					}
				} else {
					let (nd1, ctg1) = get_node(&ctg_blocks, &ctg_blk_ends, off);
					let (nd2, ctg2) = get_node(&ctg_blocks, &ctg_blk_ends, off + sz - 1);
					RNode {
						start_ctg: ctg1,
						start_base: nd1.start,
						end_ctg: ctg2,
						end_base: nd2.end,
						start_idx: off as u32
					}
				};
				nodes.push(node);
				off += sz;
			}
			nodes.push(RNode{start_idx: w1 as u32, start_ctg: 0, start_base: 0, end_ctg: 0, end_base: 0});
			start.push(nodes);
		}
		Self{n_nodes, n_items, ctg_blocks, ctg_blk_ends, offset, block_size, width, start}
	}	
	fn depth(&self) -> usize { self.width.len() }	

	pub fn write(&self, w: &mut BufWriter<File>, bbi_type: BbiBlockType, offset: u64) -> io::Result<()> {	
		let items_per_slot = if matches!(bbi_type, BbiBlockType::Bb(_)) { BB_ITEMS_PER_SLOT } else { BW_ITEMS_PER_SLOT };
		let tmp: [u32; 2] = [ 0x2468ACE0, self.block_size];
		write_u32_slice(w, &tmp)?;
		let n_ctgs = self.ctg_blocks.len();
		write_u64(w, n_ctgs as u64)?;
		
		// Find first and last bases in index
		let (ctg0, start) = self.ctg_blocks.iter().enumerate().filter(|(__, v)| !v.is_empty()).map(|(i, v)| (i as u32, v[0].start)).next().expect("No data in index");
		let (ctg1, end) = self.ctg_blocks.iter().enumerate().rev().filter(|(__, v)| !v.is_empty()).map(|(i, v)| (i as u32, v.last().unwrap().end)).next().unwrap();
		
		let tmp: [u32; 4] = [ctg0, start, ctg1, end];
		write_u32_slice(w, &tmp)?;
		write_u64(w, offset)?;
		let tmp: [u32; 2] = [ items_per_slot, 0 ];
		write_u32_slice(w, &tmp)?;
		let depth = self.depth();
		for i in 0..depth - 1 { self.write_non_leaf_level(w, depth - 1 - i)? }
		self.write_leaf_level(w)?;
		Ok(())
	}

	fn write_non_leaf_level(&self,  w: &mut BufWriter<File>, level: usize) -> io::Result<()> {
		assert!(level > 0);
		let pos = w.seek(SeekFrom::Current(0))? as u64;
		let n_nodes = self.width[level] as u64;
		let item_size = 24;
		let rn = &self.start[level];
		let n_nodes1 = self.width[level - 1] as u64; // Number of nodes at next (lower) level
		let mut off = pos + 4 * n_nodes + item_size * n_nodes1; // Offset of first node at next level
		let rn1 = &self.start[level - 1];
		let item_size1 = if level > 1 { 24 } else { 32 }; // Item size at next level
		for i in 0..n_nodes as usize {
			let a = rn[i].start_idx as usize;
			let b = rn[i + 1].start_idx as usize;			
			let tmp: [u16; 2] = [0, (b - a) as u16];
			write_u16_slice(w, &tmp)?;
			for (i, nd) in rn1[a..b].iter().enumerate() {
				write_u32_slice(w, &[nd.start_ctg, nd.start_base, nd.end_ctg, nd.end_base])?;
				write_u64(w, off)?;
				off += 4 + item_size1 + (rn1[i + 1].start_idx - nd.start_idx) as u64; 
			}
		}
		Ok(())
	}
	fn lookup(&self, off: usize) -> (usize, usize) {
		assert!(off < *self.ctg_blk_ends.last().unwrap());
		match self.ctg_blk_ends.binary_search(&off) {
			Ok(x) => self.ctg_blocks[x + 1..].iter().enumerate().filter(|(_, v)| !v.is_empty()).map(|(i, _)| (0, (i + x + 1))).next().expect("Could not find contig block"),
			Err(x) => if x > 0 { (off - self.ctg_blk_ends[x - 1], x) } else { (off, x) }
		}
	}
	
	fn write_leaf_level(&self,  w: &mut BufWriter<File>) -> io::Result<()> {
		let n_nodes = self.width[0] as u64;
		let rn = &self.start[0];
		for i in 0..n_nodes as usize {
			let a = rn[i].start_idx as usize;
			let b = rn[i + 1].start_idx as usize;
			w.write_all(&[1, 0])?;
			write_u16(w, (b - a) as u16)?;
			let (mut x, mut ctg) = self.lookup(a);
			for j in a..b {
				let nd = &self.ctg_blocks[ctg][x];
				write_u32_slice(w, &[ctg as u32, nd.start, ctg as u32, nd.end])?;
				let size = {
					if j + 1 < self.n_nodes { self.ctg_blocks[ctg][x].offset } else { self.offset }
				} - nd.offset;
				write_u64_slice(w, &[nd.offset, size])?;
				if j + 1 < a {				
					if j + 1 == self.ctg_blk_ends[ctg] {
						loop {
							x = 0;
							ctg += 1;
							// SKip over empty contigs
							if j + 1 != self.ctg_blk_ends[ctg] { break }
						}
					} else { x += 1 }
				}
			}		
		}	
		Ok(())
	}
}

fn get_node<'a>(ctg_blocks: &'a [Vec<BbiCtgBlock>], ctg_blk_ends: &[usize], off: usize) -> (&'a BbiCtgBlock, u32) {
	assert!(off < *ctg_blk_ends.last().unwrap());
	match ctg_blk_ends.binary_search(&off) {
		Ok(x) => {
			// Empty contigs can result in multiple elements in ctg_blocks having the same value
			// We need the last element in the duplicate block, which is also the first i where ctg_blocks[i] is not empty
			ctg_blocks[x + 1..].iter().enumerate().filter(|(_, v)| !v.is_empty()).map(|(i, v)| (&v[0], (i + x + 1) as u32)).next().expect("Could not find contig block")
		},
		Err(x) => if x > 0 { (&ctg_blocks[x][off - ctg_blk_ends[x - 1]], x as u32) } else { (&ctg_blocks[x][off], x as u32) }
	}
}


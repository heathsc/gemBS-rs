use std::io::{self, Seek, SeekFrom, BufWriter, Write};
use std::fs::File;
use std::convert::TryInto;

use crate::config::{VcfContig, ConfHash};
use super::print_bbi::BbiWriter;
use super::bbi_utils::*;

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
	fn depth(&self) -> usize { self.width.len() }	
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
		assert!(pos >= 4);
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

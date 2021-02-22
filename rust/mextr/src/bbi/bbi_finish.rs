use std::sync::Arc;
use std::io::{Seek, Write, SeekFrom};

use crate::config::ConfHash;
use super::write_bbi::BbiWriter;
use super::tree::RTree;
use super::bbi_zoom::{ZOOM_LEVELS, ZoomHeader};
use super::{BbiCtgBlock, BbiBlockType};
use super::bbi_file_struct::*;

/// Finish off writing of bbi file
/// This entails:
/// 
///    Generation of main index
/// 
///    For each zoom level:
///       Write out compressed zoom records
///       Generation of zoom index
/// 
///    Generation of overall summary data
///  
///    Write complete header
/// 
pub fn bbi_finish(ch: Arc<ConfHash>, mut writer: BbiWriter) {
	let bbi_type = writer.bbi_type();
	debug!("bbi_finish starting for {:?}", bbi_type);

	let (n_rec, n_zoom_rec, summary, zoom_scales) = {	
		let bbi_ref = ch.bbi().read().unwrap();
		let bbi = bbi_ref.as_ref().expect("Bbi not set");
		
		let build = match bbi_type {
			BbiBlockType::Bb(x) => bbi.bb_files()[x as usize].build.read().unwrap(),
			BbiBlockType::Bw(x) => bbi.bw_files()[x as usize].build.read().unwrap(),
		};
		( bbi.n_rec(bbi_type),
			build.n_zoom_rec(), build.summary(), build.zoom_scales()
			
		)
	};
	// Generate and write out main index
	{
		debug!("bbi_finish: generating main index for {:?}", bbi_type);
		let offset = writer.index_offset;
		let ctg_blocks = &writer.ctg_blocks;
		let rtree = RTree::init(&ctg_blocks, n_rec as u32, offset);
		rtree.write(&mut writer.fp, bbi_type, offset).expect("Error writing out main index");
	}
	
	let mut zoom_hdr = Vec::with_capacity(ZOOM_LEVELS as usize);
	// Write out zoom data and indices
	for (level, z_nrec) in n_zoom_rec.iter().enumerate() {
		debug!("bbi_finish: generating zoom level {} for {:?}", level + 1, bbi_type);
		writer.clear_ctg_blocks();
		let w = &mut writer.fp;
		let data_offset = w.seek(SeekFrom::Current(0)).unwrap() as u64;
		let zdata = &mut writer.zoom_data[level];
		let ctg_blocks = &mut writer.ctg_blocks;
		for (blk, v) in zdata.drain(..) {
			let pos = w.seek(SeekFrom::Current(0)).unwrap() as u64;
			w.write_all(&v).expect("Error writing out zoom data");
			let blocks = &mut ctg_blocks[blk.id() as usize];
			blocks.push(BbiCtgBlock::new(&blk, pos));
		}
		let index_offset = w.seek(SeekFrom::Current(0)).unwrap() as u64;
		zoom_hdr.push(ZoomHeader::new(zoom_scales[level], data_offset, index_offset));
		let rtree = RTree::init(&ctg_blocks, *z_nrec, index_offset);
		rtree.write(&mut writer.fp, bbi_type, index_offset).expect("Error writing out main index");		
	}
	
	debug!("bbi_finish: completing for {:?}", bbi_type);

	// Write out headers
	let header = &mut writer.header;
	header.set_uncompress_buf_size(ch.max_uncomp_size(bbi_type).expect("No data!") as u32);
	header.set_full_index_offset(writer.index_offset);
	header.write(&mut writer.fp).expect("Error writing out main header");
	for zhdr in zoom_hdr.iter() { zhdr.write(&mut writer.fp).expect("Error writing zoom headers"); }
	if matches!(bbi_type, BbiBlockType::Bb(_)) { write_autosql(&mut writer.fp).expect("Error writing autoSql text")}	
	summary.write(&mut writer.fp).expect("Error writing out summary table");
	header.write_ext_header(&mut writer.fp).expect("Error writing out extended header");
	
	// Fill in data count
	header.write_data_count(&mut writer.fp, n_rec).expect("Error writing out data count");
	
	// Write magic number at end of file
	header.write_terminator(&mut writer.fp).expect("Error writing out terminator");
	
	debug!("bbi_finish ending for {:?}", bbi_type);
}

use std::ffi::{CString, CStr};

use lazy_static::lazy_static;

use super::bbi_zoom::ZOOM_LEVELS;
use super::BbiBlock;

const BBI_HEADER_SIZE: u64 = 64;
const EXT_HEADER_SIZE: u64 = 64;
const ZOOM_HEADER_SIZE: u64 = 24;
const TOTAL_SUMMARY_SIZE: u64 = 40;

const _AUTOSEQ_DESC: &str = "table BisulfiteSeq
\"BED9+5 scores for bisulfite-seq data\"
\t(
\tstring\tchrom;\t\"Reference chromosome or scaffold\"
\tuint\tchromStart;\t\"Start position in chromosome\"
\tuint\tchromEnd;\t\"End position in chromosome\"
\tstring\tname;\t\"Name of item\"
\tuint\tscore;\t\"Score from 0-1000.  Capped number of reads\"
\tchar[1]\tstrand;\t\"+ or - or . for unknown\"
\tuint\tthickStart;\t\"Start of where display should be thick (start codon)\"
\tuint\tthickEnd;\t\"End of where display should be thick (stop codon)\"
\tuint\treserved;\t\"Color value R,G,B\"
\tuint\treadCount;\t\"Number of reads or coverage\"
\tuint\tpercentMeth;\t\"Percentage of reads that show methylation at this position in the genome\"
\tstring\trefContext;\t\"Reference context on strand (2 bases for CpG, 3 bases for CHG, CHH)\"
\tstring\tcalledContext;\t\"Called context on strand (2 bases for CpG, 3 bases for CHG, CHH)\"
\tuint\tgenotypeQual;\t\"Phred score for genotype call\"
\t)
";

lazy_static! {
	static ref AUTOSEQ_DESC: CString = {
		CString::new(_AUTOSEQ_DESC.as_bytes()).unwrap()		
	};
}

// Main header of bbi file
#[derive(Debug)]
#[repr(C)]
pub struct BbiHeader {
	magic: u32,
	version: u16,
	zoom_levels: u16,
	chromosome_tree_offset: u64,
	full_data_offset: u64,
	field_count: u16,
	defined_field_count: u16,
	auto_sql_offset: u64,
	total_summary_offset: u64,
	uncompress_buf_size: u32,
	extension_offset: u64,	
}

impl BbiHeader {
	pub fn chromosome_tree_offset(&self) -> u64 { self.chromosome_tree_offset }	
	pub fn set_full_data_offset(&mut self, off: u64) { self.full_data_offset = off }
	pub fn new(bb_flag: bool) -> Self {
		let version = 4;
		let zoom_levels = ZOOM_LEVELS as u16;
		let off = BBI_HEADER_SIZE + (ZOOM_LEVELS as u64) * ZOOM_HEADER_SIZE as u64;
		let (magic, field_count, defined_field_count, auto_sql_offset, total_summary_offset) = if bb_flag {
			(0x8789F2EB, 14, 0, off, off + (AUTOSEQ_DESC.to_bytes().len() + 1) as u64)
		} else {
			(0x888FFC26, 0, 0, 0, off)
		};
		let extension_offset = total_summary_offset + TOTAL_SUMMARY_SIZE;
		let chromosome_tree_offset = extension_offset + EXT_HEADER_SIZE;
		Self {magic, version, zoom_levels, chromosome_tree_offset, full_data_offset: 0, field_count, defined_field_count, 
			auto_sql_offset, total_summary_offset, uncompress_buf_size: 0, extension_offset} 
	}
} 

#[repr(C)]
pub struct BwDataHeader {
	chrom_id: u32,
	chrom_start: u32,
	chrom_end: u32,
	item_step: u32,
	item_span: u32,
	_type: u8,
	reserved: u8,
	item_count: u16,	
}

impl BwDataHeader {
	pub fn init(blk: &BbiBlock, item_count: u16) -> Self {
		Self {
			chrom_id: blk.id(),
			chrom_start: blk.start(),
			chrom_end: blk.end(),
			item_step: 0, item_span: 1,
			_type: 2, reserved: 0,
			item_count
		}
	}
}


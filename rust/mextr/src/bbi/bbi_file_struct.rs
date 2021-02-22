use std::ffi::CString;
use std::io::{self, BufWriter, Write, Seek, SeekFrom};
use std::fs::File;

use lazy_static::lazy_static;

use super::bbi_zoom::ZOOM_LEVELS;
use super::bbi_utils::*;
use super::BbiBlock;

const BBI_HEADER_SIZE: u64 = 64;
const EXT_HEADER_SIZE: u64 = 64;
const ZOOM_HEADER_SIZE: u64 = 24;
const TOTAL_SUMMARY_SIZE: u64 = 40;

const _AUTOSQL_DESC: &str = "table BisulfiteSeq
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
	static ref AUTOSQL_DESC: CString = {
		CString::new(_AUTOSQL_DESC.as_bytes()).unwrap()		
	};
}

// Main header of bbi file
#[derive(Debug)]
pub struct BbiHeader {
	magic: u32,
	version: u16,
	zoom_levels: u16,
	chromosome_tree_offset: u64,
	full_data_offset: u64,
	full_index_offset: u64,
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
	pub fn set_full_index_offset(&mut self, off: u64) { self.full_index_offset = off }
	pub fn set_uncompress_buf_size(&mut self, sz: u32) { self.uncompress_buf_size = sz }
	pub fn new(bb_flag: bool) -> Self {
		let version = 4;
		let zoom_levels = ZOOM_LEVELS as u16;
		let off = BBI_HEADER_SIZE + (ZOOM_LEVELS as u64) * ZOOM_HEADER_SIZE as u64;
		let (magic, field_count, defined_field_count, auto_sql_offset, total_summary_offset) = if bb_flag {
			(0x8789F2EB, 14, 0, off, off + (AUTOSQL_DESC.to_bytes().len() + 1) as u64)
		} else {
			(0x888FFC26, 0, 0, 0, off)
		};
		let extension_offset = total_summary_offset + TOTAL_SUMMARY_SIZE;
		let chromosome_tree_offset = extension_offset + EXT_HEADER_SIZE;
		Self {magic, version, zoom_levels, chromosome_tree_offset, full_data_offset: 0, full_index_offset: 0, field_count, 
			defined_field_count, auto_sql_offset, total_summary_offset, uncompress_buf_size: 0, extension_offset} 
	}
	pub fn write(&self, fp: &mut BufWriter<File>) -> io::Result<()> {
		fp.seek(SeekFrom::Start(0))?;
		
		// Write out main header
		write_u32(fp, self.magic)?;
		write_u16_slice(fp, &[self.version, self.zoom_levels])?;
		write_u64_slice(fp, &[self.chromosome_tree_offset, self.full_data_offset, self.full_index_offset])?;
		write_u16_slice(fp, &[self.field_count, self.defined_field_count])?;
		write_u64_slice(fp, &[self.auto_sql_offset, self.total_summary_offset])?;
		write_u32(fp, self.uncompress_buf_size)?;
		write_u64(fp, self.extension_offset)
	}
	pub fn write_ext_header(&self, fp: &mut BufWriter<File>) -> io::Result<()> {
		let pos = fp.seek(SeekFrom::Current(0))?;
		assert!(pos == self.extension_offset);
		// Size of extension offset
		write_u16(fp, EXT_HEADER_SIZE as u16)?;
		// Fill the rest with zeros
		fp.write_all(&[0; (EXT_HEADER_SIZE as usize) - 2])
	}
	pub fn write_data_count(&self, fp: &mut BufWriter<File>, data_count: u64) -> io::Result<()> {
		fp.seek(SeekFrom::Start(self.full_data_offset))?;
		write_u64(fp, data_count)
	}
	pub fn write_terminator(&self, fp: &mut BufWriter<File>) -> io::Result<()> {
		fp.seek(SeekFrom::End(0))?;
		write_u32(fp, self.magic)
	}
	
} 

pub fn write_autosql<W: Write>(w: &mut W) -> io::Result<()> { w.write_all(AUTOSQL_DESC.as_bytes_with_nul())	}

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


use std::sync::Arc;
use std::fs::File;
use std::io::{self, BufWriter, Write, Seek, SeekFrom, ErrorKind};

use crossbeam_channel::Receiver;
use zstd::block::compress;

use super::config::Config;
use super::contig::Contig;
use super::compress::CompressBlock;

pub fn new_err(s: String) -> io::Error {
	io::Error::new(ErrorKind::Other, s)	
}


/// Overall format of binary dbSNP index file
/// 
/// Main index
/// 
/// Contig data (per contig)
///   Compressed data blocks (multiple blocks per contig)
///     snp bins (multiple per data block)
/// 
/// Contig information + index (compressed)
/// 
/// 
/// Bit level format of binary dbSNP index file
/// 
/// Main Index
/// 
/// Name           Size         Description
/// ---------------------------------------------------------------------------
/// magic          32           Magic number (0xd7278434)
/// compress       8            Compression type (0 = zlib, 1 = zstd)
/// reserved       24           For future use
/// header_idx     64           File offset of contig header
/// ubuf_size      64           Maximum size of uncompressed data block
/// cheader_size   64           Compressed contig header size
/// 
/// Contig data block [ multiple blocks per contig ]
/// 
/// Name           Size         Description
/// ---------------------------------------------------------------------------
/// csize          64           Compressed size of data block (size == 0 indicates last block for contig)
/// bin            32           Bin number of first bin in block
/// cdata                       compressed data block comprising multiple snp bins (see below)
/// 
///   Snp bin (multiple per data block)
/// 
///   Name            Size             Description
///   -------------------------------------------------------------------------
///   bin_inc         1, 2, 3 or 5     Increment in bin number from previous bin * NOT INCLUDED FOR FIRST BIN IN BLOCK (assumed zero) *
/// 
///      let x = increment
/// 
///      x < 64:
///        bin_inc         8             x (increment)
///      64 < x < 256:
///        indicator       8             64
///        bin_inc         8             x
///      256 < 64 < 65536:
///        indicator       8             128
///        bin_inc         16            x
///      x > 65536:
///        indicator       8             192
///        bin_inc         32            x
/// 
///   mask0           128              bit mask for snp presence (first 128 sites)  
///   mask1           128              bit mask for snp presence (second 128 sites)    
///   rs numbers                       rs numbers for individual snps stored as packed BCD with each digit taking 4 bits (nybble), with the
///                                    first digit in the high nybble  and the second in the low nybble.
///                                    Digits 0-9 represented by bit patterns 0000 - 1001, terminating characters encoded as 1110 or 1111 for
///                                    snps that are preselected (1111) or not (1110).  Bit patterns 1010, 1011, 1100, 1101 are illegal and should not occur.
///                                    The data for adjacent snps are packed together, so a snp does not need to start on a byte boundary.
/// 
/// Contig information (compressed)
/// 
/// Name           Size         Description
/// ---------------------------------------------------------------------------
/// n_contigs       32           Number of contigs
/// contig_headers  n * 128      one header block per contig
/// 
///   Contig header block
/// 
///   Name            Size             Description
///   ---------------------------------------------------------------------------
///   min_bin         32               First non-zero bin
///   max_bin         32               Last non-zero bin
///   offset          64               File offset for start for contig data block
/// 
/// desc                         Null terminated string with description of dataset
/// contig_names                 n * null terminated strings with contig names
/// 
/// Magic Number
/// 
/// magic           32           Magic number (0xd7278434)

const IDX_MAGIC: u32 = 0xd7278434;
 
pub fn write_thread(conf: Arc<Config>, recv: Receiver<(Arc<Contig>, Vec<CompressBlock>, usize)>) {
	let output_file = conf.output().unwrap_or("dbsnp.idx");
	let mut ofile = match File::create(output_file) {
		Ok(f) => BufWriter::new(f),
		Err(e) => panic!("Couldn't open output file {}: {}", output_file, e),
	};
	// Skip over header block (we will fill it in at the end);
	ofile.seek(SeekFrom::Start(32)).expect("IO error - can not seek");
	// Make list of contigs
	let mut ctgs = Vec::new();
	// Track maximum uncompressed buffer size
	let mut max_size = 0;
	for (ctg, mut cblock, msize) in recv.iter() {
		info!("Writing out data for contig {}", ctg.name());
		let pos = ofile.seek(SeekFrom::Current(0)).expect("IO error - can't get current file position");
		debug!("Writer thread received data for contig {}, file pos = {}, starting writing", ctg.name(), pos);
		for cb in cblock.drain(..) {
			let cbuf = cb.cbuf();
			write_u64(ofile.by_ref(), &[cbuf.len() as u64]).expect("Write error");
			write_u32(ofile.by_ref(), &[cb.first_bin()]).expect("Write error");
			ofile.write_all(cbuf).expect("Write error");
		}
		debug!("Writer thread finished writing out data for contig {}", ctg.name());
		ctgs.push((ctg.clone(), pos));
		max_size = max_size.max(msize);
	}
	debug!("Writer thread adding index information");
	let pos = ofile.seek(SeekFrom::Current(0)).expect("IO error - can't get current file position");
	let mut ubuf: Vec<u8> = Vec::new();
	write_u32(ubuf.by_ref(), &[ctgs.len() as u32]).expect("Write error");
	for (ctg, x) in ctgs.iter() {
		let cdata = ctg.data().read().unwrap();
		let (min, max) = cdata.min_max().unwrap();
		write_u32(ubuf.by_ref(), &[min as u32, max as u32]).expect("Write error");
		write_u64(ubuf.by_ref(), &[*x]).expect("Write error");
	}
	let desc = conf.description().unwrap_or_else(|| "track name = dbSNP_index description = \"dbSNP index produced by dbSNP_idx\"".to_string());
	ubuf.write_all(desc.as_bytes()).expect("Write error");
	ubuf.write_all(&[0]).expect("Write error");
	for (ctg, _) in ctgs.iter() {
		ubuf.write_all(ctg.name().as_bytes()).expect("Write error");
		ubuf.write_all(&[0]).expect("Write error");	
	}
	let cbuf = compress(&ubuf, 0).expect("Error when compressing");
	max_size = max_size.max(cbuf.len());
	ofile.write_all(&cbuf).expect("Write error");
	write_u32(ofile.by_ref(), &[IDX_MAGIC]).expect("Write error");
	ofile.seek(SeekFrom::Start(0)).expect("IO error - can't seek to start of output file");
	write_u32(ofile.by_ref(), &[IDX_MAGIC]).expect("Write error");
	ofile.write_all(&[8,0,0,0]).expect("Write error");
	write_u64(ofile.by_ref(), &[pos, max_size as u64, cbuf.len() as u64]).expect("Write error");
	debug!("Writer thread terminating");
}

pub fn write_u16<W: Write>(mut file: W, buf: &[u16]) -> io::Result<()> {
	for x in buf.iter() {
		let p = x.to_le_bytes();
		let n = file.write(&p)?;
		if n != 2 { return Err(new_err("Error reading from file".to_string())) }
	}
	Ok(())
}

pub fn write_u32<W: Write>(mut file: W, buf: &[u32]) -> io::Result<()> {
	for x in buf.iter() {
		let p = x.to_le_bytes();
		let n = file.write(&p)?;
		if n != 4 { return Err(new_err("Error reading from file".to_string())) }
	}
	Ok(())
}

pub fn write_u64<W: Write>(mut file: W, buf: &[u64]) -> io::Result<()> {
	for x in buf.iter() {
		let p = x.to_le_bytes();
		let n = file.write(&p)?;
		if n != 8 { return Err(new_err("Error reading from file".to_string())) }
	}
	Ok(())
}

pub fn write_u128<W: Write>(mut file: W, buf: &[u128]) -> io::Result<()> {
	for x in buf.iter() {
		let p = x.to_le_bytes();
		let n = file.write(&p)?;
		if n != 16 { return Err(new_err("Error reading from file".to_string())) }
	}
	Ok(())
}

use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufRead, BufWriter, Error, ErrorKind, Result, stdin};
use std::process::{Command, Stdio, ChildStdout, ChildStdin};
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use super::find_exec_path;

lazy_static! {
	pub static ref GZIP_PATH: Option<PathBuf> = find_exec_path("gzip");
	pub static ref PIGZ_PATH: Option<PathBuf> = find_exec_path("pigz");
	pub static ref XZ_PATH: Option<PathBuf> = find_exec_path("xz");
	pub static ref BZIP2_PATH: Option<PathBuf> = find_exec_path("bzip2");
	pub static ref PBZIP2_PATH: Option<PathBuf> = find_exec_path("pbzip2");
	pub static ref ZSTD_PATH: Option<PathBuf> = find_exec_path("zstd");
	pub static ref LZ4_PATH: Option<PathBuf> = find_exec_path("lz4");
	pub static ref LZMA_PATH: Option<PathBuf> = find_exec_path("lzma");
}

#[derive(Debug)]
pub enum CompressType {
    GZIP,
    COMPRESS,
    BZIP2,
    XZ,
	ZSTD,
	LZ4,
	LZMA,
    UNCOMPRESSED,
}

fn get_path<'a>(x: Option<&'a PathBuf>, s: &'static str) -> Result<&'a PathBuf> {
	x.ok_or_else(|| Error::new(ErrorKind::Other, format!("Can not find {} executable to uncompress file", s)))
}

impl CompressType {
	pub fn get_exec_path(&self) -> Result<&PathBuf> {
		match self {
			CompressType::GZIP | CompressType::COMPRESS => get_path(PIGZ_PATH.as_ref().or_else(|| GZIP_PATH.as_ref()).or_else(|| ZSTD_PATH.as_ref()), "pigz, gzip or zstd"),
			CompressType::BZIP2 => get_path(PBZIP2_PATH.as_ref().or_else(|| BZIP2_PATH.as_ref()), "pbzip2 or bzip2"),
			CompressType::XZ => get_path(XZ_PATH.as_ref().or_else(|| ZSTD_PATH.as_ref()), "xz or zstd"),
			CompressType::LZ4 => get_path(LZ4_PATH.as_ref().or_else(|| ZSTD_PATH.as_ref()), "lz4 or zstd"),
			CompressType::LZMA => get_path(LZMA_PATH.as_ref().or_else(|| ZSTD_PATH.as_ref()), "lzma or zstd"),
			CompressType::ZSTD => get_path(ZSTD_PATH.as_ref(), "zstd"),
			CompressType::UNCOMPRESSED => Err(Error::new(ErrorKind::Other, "Can not get filter path for uncompressed file".to_string())),
		}
	}	
}

pub enum ReadType {
	Pipe(ChildStdout),
	File(File),	
}

pub fn open_read_filter<P: AsRef<Path>, I, S>(prog: P, args: I) -> Result<ChildStdout> 
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>, 
{
	let path: &Path = prog.as_ref();
	match Command::new(path).args(args).stdout(Stdio::piped()).spawn() {
		Ok(proc) => Ok(proc.stdout.expect("pipe problem")),
		Err(error) => Err(Error::new(ErrorKind::Other, format!("Error executing pipe command '{}': {}", path.display(), error))),
	}
}

pub fn new_read_filter_from_pipe<P: AsRef<Path>>(prog: P, pipe: Stdio) -> Result<ChildStdout> {
	let path: &Path = prog.as_ref();
    match Command::new(path).arg("-d")
        .stdin(pipe)
        .stdout(Stdio::piped())
        .spawn() {
            Ok(proc) => Ok(proc.stdout.expect("pipe problem")),
            Err(error) => Err(Error::new(ErrorKind::Other, format!("Error executing pipe command '{} -d': {}", path.display(), error))),
        }
}

pub fn open_write_filter<P: AsRef<Path>, I, S>(file: std::fs::File, prog: P, args: I) -> Result<ChildStdin> 
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>, 
{
	let path: &Path = prog.as_ref();
	match Command::new(path).args(args).stdout(file).stdin(Stdio::piped()).spawn() {
		Ok(proc) => Ok(proc.stdin.expect("pipe problem")),
		Err(error) => Err(Error::new(ErrorKind::Other, format!("Error exectuing pipe command '{}': {}", path.display(), error))),
	}
}

fn test_open_file(path: &Path) -> Result<std::fs::File> {
    match File::open(path) {
        Ok(handle) => Ok(handle),
        Err(error) => Err(Error::new(ErrorKind::Other, format!("Error opening {} for input: {}", path.display(), error))),
    }
}

fn get_compress_type(path: &Path) -> Result<CompressType> {
    let mut f = test_open_file(path)?;
    let mut buf = [0; 6];
    let n = match f.read(&mut buf) {
        Ok(num) => num,
        Err(error) => return Err(Error::new(ErrorKind::Other, format!("Error reading from {}: {}", path.display(), error))),
    };
    
    let mut ctype = CompressType::UNCOMPRESSED;    
    if n == 6 {
        if buf[0] == 0x1f {
            if buf[1] == 0x9d {
                ctype = CompressType::COMPRESS;
            } else if buf[1] == 0x8b && buf[2] == 0x08 {
                ctype = CompressType::GZIP;
            }
        } else if buf[0] == b'B' && buf[1] == b'Z' && buf[2] == b'h' && buf[3] >= b'0' && buf[3] <= b'9' {
            ctype = CompressType::BZIP2;
        } else if buf[0] == 0xfd && buf[1] == b'7' && buf[2] == b'z' && buf[3] == b'X' && buf[4] == b'Z' && buf[5] == 0x00 {
            ctype = CompressType::XZ;
        } else if buf[0] == 0x28 && buf[1] == 0xB5 && buf[2] == 0x2F && buf[3] == 0xFD {
			ctype = CompressType::ZSTD;
        } else if buf[0] == 0x04 && buf[1] == 0x22 && buf[2] == 0x4D && buf[3] == 0x18 {
			ctype = CompressType::LZ4;
        } else if buf[0] == 0x5D && buf[1] == 0x0 && buf[2] == 0x0 {
			ctype = CompressType::LZMA;
		} 
    }
    Ok(ctype)
}

pub fn open_reader<P: AsRef<Path>>(name: P) -> Result<ReadType> {
	let ctype = get_compress_type(name.as_ref())?;
	let f = test_open_file(name.as_ref())?;
	match ctype {
		CompressType::UNCOMPRESSED => Ok(ReadType::File(f)),
		_ => new_read_filter_from_pipe(ctype.get_exec_path()?, Stdio::from(f)).map(ReadType::Pipe),
	}
}

pub fn open_bufreader<P: AsRef<Path>>(name: P) -> Result<Box<dyn BufRead>> {
	match open_reader(name)? {
		ReadType::File(file) => Ok(Box::new(BufReader::new(file))),
		ReadType::Pipe(pipe) => Ok(Box::new(BufReader::new(pipe))),
	}
}

pub fn get_reader<P: AsRef<Path>>(name: Option<P>) -> Result<Box<dyn BufRead>> {
    match name {
        Some(file) => open_bufreader(file),
        None => Ok(Box::new(BufReader::new(stdin()))),
    }
}

pub fn open_bufwriter<P: AsRef<Path>>(path: P) -> Result<Box<dyn Write>> {
	let file = File::create(path)?;
	Ok(Box::new(BufWriter::new(file)))
}

pub fn open_pipe_writer<P: AsRef<Path>, Q: AsRef<Path>, I, S>(path: P, prog: Q, args: I) -> Result<Box<dyn Write>> 
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>, 
{
	let file = File::create(path)?;
	Ok(Box::new(BufWriter::new(open_write_filter(file, prog, args)?)))	
}

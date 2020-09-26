use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufRead, BufWriter, Error, ErrorKind, Result};
use std::process::{Command, Stdio, ChildStdout, ChildStdin};
use std::path::Path;
use std::ffi::OsStr;

// use flate2::read::GzDecoder;

#[derive(Debug)]
pub enum CompressType {
    GZIP,
    COMPRESS,
    BZIP2,
    XZ,
    UNCOMPRESSED,
}

pub enum ReadType {
	Pipe(ChildStdout),
	File(File),	
//	FlateGz(File),
}

pub fn open_read_filter<P: AsRef<Path>, I, S>(prog: P, args: I) -> Result<ChildStdout> 
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>, 
{
	let path: &Path = prog.as_ref();
	match Command::new(path).args(args).stdout(Stdio::piped()).spawn() {
		Ok(proc) => Ok(proc.stdout.expect("pipe problem")),
		Err(error) => Err(Error::new(ErrorKind::Other, format!("Error exectuing pipe command '{}': {}", path.display(), error))),
	}
}

pub fn new_read_filter_from_pipe<P: AsRef<Path> + std::fmt::Debug + Copy>(prog: P, pipe: Stdio) -> Result<ChildStdout> {
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
        }
    }
    Ok(ctype)
}

pub fn open_reader<P: AsRef<Path>>(name: P) -> Result<ReadType> {
	let ctype = get_compress_type(name.as_ref())?;
	let f = test_open_file(name.as_ref())?;
	match ctype {
		CompressType::UNCOMPRESSED => Ok(ReadType::File(f)),
       	CompressType::BZIP2 => new_read_filter_from_pipe(&"bzip", Stdio::from(f)).map(ReadType::Pipe),
        CompressType::XZ => new_read_filter_from_pipe(&"xz", Stdio::from(f)).map(ReadType::Pipe),
//		CompressType::GZIP => Ok(ReadType::FlateGz(f)),
        _ => new_read_filter_from_pipe(&"gzip", Stdio::from(f)).map(ReadType::Pipe),
	}
}

pub fn open_bufreader<P: AsRef<Path>>(name: P) -> Result<Box<dyn BufRead>> {
	match open_reader(name)? {
		ReadType::File(file) => Ok(Box::new(BufReader::new(file))),
		ReadType::Pipe(pipe) => Ok(Box::new(BufReader::new(pipe))),
//		ReadType::FlateGz(file) => Ok(Box::new(BufReader::new(GzDecoder::new(file)))),
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

use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter, Error, ErrorKind, stdin, stdout, Result};
use std::process::{Command, Stdio, ChildStdout};
use std::path::Path;

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
}

pub fn new_filter_from_pipe(prog: &str, pipe: Stdio) -> Result<ChildStdout> {

    match Command::new(prog).arg("-d")
        .stdin(pipe)
        .stdout(Stdio::piped())
        .spawn() {
            Ok(proc) => Ok(proc.stdout.expect("pipe problem")),
            Err(error) => Err(Error::new(ErrorKind::Other, format!("Error executing pipe command '{} -d': {}", prog, error))),
        }
}

fn test_open_file(path: &Path) -> Result<std::fs::File> {
    let name = path.to_string_lossy();
    match File::open(path) {
        Ok(handle) => Ok(handle),
        Err(error) => Err(Error::new(ErrorKind::Other, format!("Error opening '{}' for input: {}", name, error))),
    }
}

fn get_compress_type(path: &Path) -> Result<CompressType> {
    let mut f = test_open_file(path)?;
    let mut buf = [0; 6];
    let n = match f.read(&mut buf) {
        Ok(num) => num,
        Err(error) => return Err(Error::new(ErrorKind::Other, format!("Error reading from '{}': {}", path.to_string_lossy(), error))),
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

pub fn open_reader(name: &Path) -> Result<ReadType> {
	let ctype = get_compress_type(name)?;
	let f = test_open_file(name)?;
	match ctype {
		CompressType::UNCOMPRESSED => Ok(ReadType::File(f)),
       	CompressType::BZIP2 => new_filter_from_pipe(&"bzip", Stdio::from(f)).map(ReadType::Pipe),
        CompressType::XZ => new_filter_from_pipe(&"xz", Stdio::from(f)).map(ReadType::Pipe),
        _ => new_filter_from_pipe(&"gzip", Stdio::from(f)).map(ReadType::Pipe),
	}
}

pub fn open_bufreader(name: &Path) -> Result<Box<dyn BufRead>> {
	match open_reader(name)? {
		ReadType::File(file) => Ok(Box::new(BufReader::new(file))),
		ReadType::Pipe(pipe) => Ok(Box::new(BufReader::new(pipe))),
	}
}

pub fn open_bufwriter(path: &Path) -> Result<Box<dyn Write>> {
	let file = File::create(path)?;
	Ok(Box::new(BufWriter::new(file)))
}

pub fn get_reader(name: Option<&str>) -> Result<Box<dyn BufRead>> {
    match name {
        Some(file) => open_bufreader(Path::new(file)),
        None => Ok(Box::new(BufReader::new(stdin()))),
    }
}

pub fn get_writer(name: Option<&str>, filter: Option<&str>) -> Result<Box<dyn Write>> {
    let open_file = | x: &str | {
        let path = Path::new(x);
        match File::create(&path) {
            Err(why) => panic!("couldn't open {}: {}", path.display(), why),
            Ok(file) => file,        
        }
    };
        
    let pipe: Box<dyn Write> = match filter {
        Some(prog) => {
            let child = match name {
                Some(file) => {
                    Command::new(prog).stdin(Stdio::piped()).stdout(Stdio::from(open_file(file))).spawn()
                },
                None =>  Command::new(prog).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()
            };
            match child {
                Err(why) => panic!("couldn't spawn {}: {}", prog, why),
                Ok(mut process) => Box::new(BufWriter::new(process.stdin.take().unwrap()))
            }
        },
        None => match name {
            Some(file) => Box::new(BufWriter::new(open_file(file))),
            None => Box::new(BufWriter::new(stdout()))
        },
    };
    Ok(pipe)
}

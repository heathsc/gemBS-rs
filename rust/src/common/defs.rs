use std::str::FromStr;
use std::fmt;
use std::convert::From;
use serde::{Deserialize, Serialize};
use regex::Regex;
use lazy_static::lazy_static;

use crate::config::contig;
use super::latex_utils::PageSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Section {
	Default, Index, Mapping, Calling, Extract, Report, MD5Sum,
}

impl FromStr for Section {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(Section::Default),
            "index" => Ok(Section::Index),
            "mapping" => Ok(Section::Mapping),
            "calling" => Ok(Section::Calling),
            "extract" => Ok(Section::Extract),
            "report" => Ok(Section::Report),
            "md5sum" => Ok(Section::MD5Sum),
            _ => Err("no match"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContigInfo {
	Contigs, ContigPools,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContigData {
	Contig(contig::Contig),
	ContigPool(contig::ContigPool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Metadata { SampleBarcode, SampleName, LibraryBarcode, Dataset, AltDataset, FileType,
	FilePath, FilePath1, FilePath2, ReadEnd, Description, Centre, Platform,	Bisulfite,
}

impl FromStr for Metadata {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut st = s.to_lowercase();
		st.retain(|c| c != '_');
        match st.as_str() {
            "sampleid" | "barcode" | "samplebarcode" => Ok(Metadata::SampleBarcode),
            "sample" | "name" | "samplename" => Ok(Metadata::SampleName),
			"library" | "librarybarcode" | "lib" | "libbarcode"  => Ok(Metadata::LibraryBarcode),
			"dataset" | "fileid" | "fli" => Ok(Metadata::Dataset),
			"filetype" | "type" => Ok(Metadata::FileType),
			"file" | "location" | "path" | "command" => Ok(Metadata::FilePath),
			"file1" | "end1" | "location1" | "path1" | "command1" => Ok(Metadata::FilePath1),
			"file2" | "end2" | "location2" | "path2" | "command2" => Ok(Metadata::FilePath2),
			"readend" | "read_end" | "read" | "end" => Ok(Metadata::ReadEnd),
			"description" | "desc" => Ok(Metadata::Description),
			"centre" | "center" => Ok(Metadata::Centre),
			"platform" => Ok(Metadata::Platform),
			"bisulfite" | "bisulphite" | "bis" => Ok(Metadata::Bisulfite),
            _ => Err("no match"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileType { Paired, Interleaved, Single, BAM, Stream}

impl FromStr for FileType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "paired" => Ok(FileType::Paired),
            "interleaved" => Ok(FileType::Interleaved),
            "single" | "unpaired" => Ok(FileType::Single),
            "bam"| "sam" => Ok(FileType::BAM),
            _ => Err("FileType: no match"),
        }
    }
}

impl fmt::Display for FileType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			FileType::Paired => write!(f, "paired"),
			FileType::Interleaved => write!(f, "paired"),
			FileType::Single => write!(f, "single"),
			FileType::BAM => write!(f, "bam"),
			FileType::Stream => write!(f, "stream"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MemSize {
	mem: usize,	
}

impl MemSize {
	pub fn mem(&self) -> usize { self.mem }
}

impl FromStr for MemSize {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
		lazy_static! { static ref RE: Regex = Regex::new(r"^(\d+)([kKmMgG]?)$").unwrap(); }
		let (mem, success) = {
			if let Some(cap) = RE.captures(s) {
				if let Ok(a) = <usize>::from_str(cap.get(1).unwrap().as_str()) {
					let fact = if let Some(y) = cap.get(2) {
						match y.as_str() {
							"k" | "K" => 0x400,
							"m" | "M" => 0x100000,
							"g" | "G" => 0x40000000,
							_ => return Err("Invalid memory string"),
						}
					} else { 1 };
					(a * fact, true)
				} else { (0, false) }
			} else { (0, false) }
		};
		if success { Ok(MemSize{mem}) } else { Err("Invalid memory string") }
	}
}

impl fmt::Display for MemSize {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mem = self.mem;
		if mem.trailing_zeros() >= 30 { write!(f, "{}G", mem >> 30) }
		else if mem.trailing_zeros() >= 20 { write!(f, "{}M", mem >> 20) }
		else if mem.trailing_zeros() >= 10 { write!(f, "{}M", mem >> 10) }
		else { write!(f, "{}", mem) }
	}	
}

impl From<usize> for MemSize {
	fn from(mem: usize) -> Self { MemSize{mem}}	
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct JobLen {
	secs: usize,	
}

impl FromStr for JobLen {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
		lazy_static! { static ref RE: Regex = Regex::new(r"^(\d+-)?(\d+:)?(\d+:)?(\d+)$").unwrap(); }
		if let Some(cap) = RE.captures(s) {
			let mut v = [None; 4];			
			if let Some(s) = cap.get(1) { if let Ok(x) = <usize>::from_str(s.as_str().trim_end_matches('-')) { v[0] = Some(x); } else { return Err("Invalid time") } }
			if let Some(s) = cap.get(2) { if let Ok(x) = <usize>::from_str(s.as_str().trim_end_matches(':')) { v[1] = Some(x); } else { return Err("Invalid time") } }
			if let Some(s) = cap.get(3) { if let Ok(x) = <usize>::from_str(s.as_str().trim_end_matches(':')) { v[2] = Some(x); } else { return Err("Invalid time") } }
			if let Some(s) = cap.get(4) { if let Ok(x) = <usize>::from_str(s.as_str()) { v[3] = Some(x); } else { return Err("Invalid time") } }

			match v[..] {
				[None, None, None, Some(mins)] if mins <= 60 => Ok(JobLen{secs: mins * 60}),
				[None, Some(mins), None, Some(secs)] if mins <= 60 && secs <= 60 =>  Ok(JobLen{secs: mins * 60 + secs}),
				[None, Some(hrs), Some(mins), Some(secs)] if hrs <= 24 && mins <= 60 && secs <= 60 => Ok(JobLen{secs: hrs * 3600 + mins * 60 + secs}),
				[Some(days), None, None, Some(hrs)] if hrs <= 24 => Ok(JobLen{secs: (days * 24 + hrs) * 3600 }),
				[Some(days), Some(hrs), None, Some(mins)] if hrs <= 24 && mins <= 60 => Ok(JobLen{secs: (days * 24 * 60 + hrs * 60 + mins) * 60 }),
				[Some(days), Some(hrs), Some(mins), Some(secs)] if hrs <= 24 && mins <= 60 && secs <= 60 => Ok(JobLen{secs: days * 24 * 3600 + hrs * 3600 + mins * 60 + secs}),
				_ => Err("Invalid time"),
			}
		} else { Err("Invalid time") }
    }
}

impl fmt::Display for JobLen {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let mut secs = self.secs;
		let days = secs / (24 * 3600);
		secs %= 24 * 3600;
		let hrs = secs / 3600;
		secs %= 3600;
		let mins = secs / 60;
		secs %= 60;
		match (days, hrs, mins, secs) {
			(0, 0, min, 0) => write!(f, "{}", min),
			(0, 0, min, sec) => write!(f, "{}:{}", min, sec),
			(0, hr, min, sec) => write!(f, "{}:{}:{}", hr, min, sec),
			(day, hr, 0, 0) => write!(f, "{}-{}", day, hr),
			(day, hr, min, 0) => write!(f, "{}-{}:{}", day, hr, min),
			(day, hr, min, sec) => write!(f, "{}-{}:{}:{}", day, hr, min, sec),
		} 
	}	
}

impl From<usize> for JobLen {
	fn from(secs: usize) -> Self { JobLen{secs}}	
}


#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ReadEnd { End1, End2 }

impl FromStr for ReadEnd {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {		
        match s.to_lowercase().as_str() {
			"1" | "end1" => Ok(ReadEnd::End1),
			"2" | "end2" => Ok(ReadEnd::End2),
            _ => Err("ReadEnd: no match"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataValue {
	String(String),
	StringVec(Vec<String>),	
	ReadEnd(ReadEnd),
	FileType(FileType),
	Bool(bool),
	Int(isize),	
	Float(f64),
	FloatVec(Vec<f64>),
	JobLen(JobLen),
	PageSize(PageSize),
	MemSize(MemSize),
}

impl DataValue {
	pub fn from_str(s: &str, vtype: VarType) -> Result<Self, String> {
		match vtype {
			VarType::String | VarType::StringVec => Ok(DataValue::String(s.to_string())),
			VarType::ReadEnd => Ok(DataValue::ReadEnd(s.parse::<ReadEnd>()?)),
			VarType::JobLen => Ok(DataValue::JobLen(s.parse::<JobLen>()?)),
			VarType::PageSize => Ok(DataValue::PageSize(s.parse::<PageSize>()?)),
			VarType::FileType => Ok(DataValue::FileType(s.parse::<FileType>()?)),
			VarType::MemSize => Ok(DataValue::MemSize(s.parse::<MemSize>()?)),
			VarType::Bool => match s.to_lowercase().as_str() {
				"false" | "no" | "0" => Ok(DataValue::Bool(false)),
				"true" | "yes" | "1" => Ok(DataValue::Bool(true)),
				_ => Err(format!("Could not parse {} as boolean value", s)),
			},
			VarType::Int => match s.parse::<isize>() {
				Ok(x) => Ok(DataValue::Int(x)),
				Err(_) => Err(format!("Could not parse {} as integer value", s)),
			},
			VarType::Float | VarType::FloatVec => match s.parse::<f64>() {
				Ok(x) => Ok(DataValue::Float(x)),
				Err(_) => Err(format!("Could not parse {} as float value", s)),
			},
		}
	} 	
}

#[derive(Debug, Clone, Copy)]
pub enum VarType {
	String, StringVec, Bool, Int, Float, FloatVec, ReadEnd, FileType, JobLen, PageSize, MemSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
	Index, Map, MergeBams, Call, MergeBcfs, Extract, MapReport, CallReport, MD5Sum,	IndexBcf, MergeCallJsons
}

impl fmt::Display for Command {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Command::Index => write!(f, "index"),
			Command::Map => write!(f, "map"),
			Command::MergeBams => write!(f, "merge-bams"),
			Command::Call => write!(f, "call"),
			Command::MergeBcfs => write!(f, "merge-bcfs"),
			Command::Extract => write!(f, "extract"),
			Command::MapReport => write!(f, "map-report"),
			Command::CallReport => write!(f, "call-report"),
			Command::MD5Sum => write!(f, "md5sum"),
			Command::IndexBcf => write!(f, "index-bcf"),
			Command::MergeCallJsons => write!(f, "merge-call-jsons"),
		}
	}
}

pub const SIGTERM: usize = signal_hook::SIGTERM as usize;
pub const SIGINT: usize = signal_hook::SIGINT as usize;
pub const SIGQUIT: usize = signal_hook::SIGQUIT as usize;		
pub const SIGHUP: usize = signal_hook::SIGHUP as usize;

pub const CONTIG_POOL_SIZE: usize = 25_000_000;

pub fn signal_msg(sig: usize) -> &'static str {
	match sig {
		SIGTERM => "SIGTERM",
		SIGINT => "SIGINT",
		SIGHUP => "SIGHUP",
		SIGQUIT => "SIGQUIT",
		_ => "UNKNOWN",
	}
}	

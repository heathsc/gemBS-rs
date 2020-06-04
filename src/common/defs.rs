use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::config::contig;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Section {
	Default, Index, Mapping, Calling, Extract, Report,
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
        match s.to_lowercase().as_str() {
            "sample_barcode" | "sampleid" | "barcode" | "samplebarcode" => Ok(Metadata::SampleBarcode),
            "sample_name" | "sample" | "name" | "samplename" => Ok(Metadata::SampleName),
			"library_barcode" | "library" | "librarybarcode" | "lib" | "libbarcode"  => Ok(Metadata::LibraryBarcode),
			"dataset" | "fileid" | "fli" => Ok(Metadata::Dataset),
			"file_type" | "filetype" | "type" => Ok(Metadata::FileType),
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
}

impl DataValue {
	pub fn from_str(s: &str, vtype: VarType) -> Result<Self, String> {
		match vtype {
			VarType::String | VarType::StringVec => Ok(DataValue::String(s.to_string())),
			VarType::ReadEnd => Ok(DataValue::ReadEnd(s.parse::<ReadEnd>()?)),
			VarType::FileType => Ok(DataValue::FileType(s.parse::<FileType>()?)),
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
	String, StringVec, Bool, Int, Float, FloatVec, ReadEnd, FileType,
}

pub const SIGTERM: usize = signal_hook::SIGTERM as usize;
pub const SIGINT: usize = signal_hook::SIGINT as usize;
pub const SIGQUIT: usize = signal_hook::SIGQUIT as usize;		
pub const SIGHUP: usize = signal_hook::SIGHUP as usize;



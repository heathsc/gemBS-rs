use std::collections::HashMap;
use std::clone::Clone;
use std::convert::From;
use std::ops::{Index, IndexMut, AddAssign};
use std::io::Read;
use serde::{Deserialize};

//type Counts = [usize; 2];
//type Count = [usize; 1];

#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Counts([usize; 2]);


impl AddAssign for Counts {
    fn add_assign(&mut self, other: Self) {
		self[0] += other[0];
		self[1] += other[1];
    }
}

impl Index<usize> for Counts {
    type Output = usize;
    fn index(&self, index: usize) -> &Self::Output { &self.0[index] }
} 
impl IndexMut<usize> for Counts {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output { &mut self.0[index] }
} 

impl Counts { fn new() -> Self { Counts([0;2]) } }

#[derive(Debug, Copy, Clone, Deserialize)]
struct Count([usize; 1]);
impl Index<usize> for Count {
    type Output = usize;
    fn index(&self, index: usize) -> &Self::Output { &self.0[index] }
} 
impl IndexMut<usize> for Count {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output { &mut self.0[index] }
} 

impl AddAssign for Count {
    fn add_assign(&mut self, other: Self) { self[0] += other[0]; }
}
impl Count { fn new() -> Self { Count([0; 1])} }

#[derive(Clone, Deserialize)]
enum MapperType {
	Single, Paired, Unknown
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Reads<T> {
	general: T,
	unmapped: T,
	sequencing_control: Option<T>,
	under_conversion_control: Option<T>,
	over_conversion_control: Option<T>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
struct NumReadsBS<T> {
	c2t: T,
	g2a: T,
}

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BaseCounts<T> {
	pub a: T,
	pub c: T,
	pub g: T,
	pub t: T,
	pub n: T,	
}
impl AddAssign for BaseCounts<Counts> {
    fn add_assign(&mut self, other: BaseCounts<Counts>) {
		self.a += other.a;
		self.c += other.c;
		self.g += other.g;
		self.t += other.t;
		self.n += other.n;
    }
}
impl BaseCounts<Counts> {
	pub fn new() -> Self { Self{ a: Counts::new(), c:Counts::new(), g: Counts::new(), t: Counts::new(), n: Counts::new() }}
}
impl BaseCounts<Count> {
	pub fn new() -> Self { Self{ a: Count::new(), c:Count::new(), g: Count::new(), t: Count::new(), n: Count::new() }}
}
impl From<Count> for Counts {
	fn from(c: Count) -> Self { Counts([c[0], 0]) }
}
impl From<BaseCounts<Count>> for BaseCounts<Counts> {
	fn from(bc: BaseCounts<Count>) -> Self {
		BaseCounts{ a: bc.a.into(), c: bc.c.into(), g: bc.g.into(), t: bc.t.into(), n: bc.n.into() }
	}	
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BaseCountStats<T> {
	overall: BaseCounts<T>,
	#[serde(rename = "GeneralC2T")]
	general_c2t: Option<BaseCounts<T>>,
	#[serde(rename = "GeneralG2A")]
	general_g2a: Option<BaseCounts<T>>,
	#[serde(rename = "UnderConversionControlC2T")]
	under_conversion_control_c2t: Option<BaseCounts<T>>,
	#[serde(rename = "UnderConversionControlG2A")]
	under_conversion_control_g2a: Option<BaseCounts<T>>,
	#[serde(rename = "OverConversionControlC2T")]
	over_conversion_control_c2t: Option<BaseCounts<T>>,
	#[serde(rename = "OverConversionControlG2A")]
	over_conversion_control_g2a: Option<BaseCounts<T>>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Paired {
	read_group: Option<String>,
	reads: Reads<Counts>,
	#[serde(rename = "NumReadsBS")]
	num_reads_bs: Option<NumReadsBS<Counts>>,
	correct_pairs: usize,
	base_counts: BaseCountStats<Counts>,
	hist_mapq: Vec<usize>,
	hist_read_len: [HashMap<String, usize>; 2],
	hist_mismatch: [HashMap<String, usize>; 2],
	hist_template_len: HashMap<String, usize>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Single {
	read_group: Option<String>,
	reads: Reads<Count>,
	#[serde(rename = "NumReadsBS")]
	num_reads_bs: NumReadsBS<Count>,
	base_counts: BaseCountStats<Count>,
	hist_mapq: Vec<usize>,
	hist_read_len: [HashMap<String, usize>; 1],
	hist_mismatch: [HashMap<String, usize>; 1],
}

#[derive(Clone, Deserialize)]
#[serde(tag = "MapperType", rename_all = "PascalCase")]
pub enum MapJson {
	Paired(Paired),
	Unknown(Paired),
	Single(Single),
} 

impl MapJson {
	pub fn from_reader<T: Read>(rdr: T) -> Result<Self, String> {
		serde_json::from_reader(rdr).map_err(|e| format!("Couldn't parse call JSON file {}", e))
	}
	pub fn get_conversion_counts(&self) -> (BaseCounts<Counts>, BaseCounts<Counts>) {
		let mut ct1 = BaseCounts::<Counts>::new();
		let mut ct2 = BaseCounts::<Counts>::new();
		match self {
			MapJson::Paired(s) | MapJson::Unknown(s) => {
				if let Some(bc) = s.base_counts.under_conversion_control_c2t { ct1 = bc; }
				if let Some(bc) = s.base_counts.under_conversion_control_g2a { ct1 += bc; }
				if let Some(bc) = s.base_counts.over_conversion_control_c2t { ct2 = bc; }
				if let Some(bc) = s.base_counts.over_conversion_control_g2a { ct2 += bc; }
			},
			MapJson::Single(s) => {
				if let Some(bc) = s.base_counts.under_conversion_control_c2t { ct1 = bc.into(); }
				if let Some(bc) = s.base_counts.under_conversion_control_g2a { ct1 += bc.into(); }
				if let Some(bc) = s.base_counts.over_conversion_control_c2t { ct2 = bc.into(); }
				if let Some(bc) = s.base_counts.over_conversion_control_g2a { ct2 += bc.into(); }				
			},
		}
		(ct1, ct2)
	}
}

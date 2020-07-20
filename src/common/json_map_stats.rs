use std::collections::HashMap;
use std::clone::Clone;
use std::convert::From;
use std::ops::{Index, IndexMut, Add, AddAssign};
use std::io::Read;
use serde::{Deserialize};
use super::json_call_stats::add_assign_vec;

pub trait New { fn new() -> Self; }

#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Counts(pub [usize; 2]);

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

impl New for Counts { fn new() -> Self { Counts([0;2]) } }

#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Count(pub [usize; 1]);
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
impl Add for Count {
    type Output = Self;
    fn add(self, other: Self) -> Self { Self([self[0] + other[0]]) }
}
impl Add for Counts {
    type Output = Self;
    fn add(self, other: Self) -> Self { Self([self[0] + other[0], self[1] + other[1]]) }
}

impl New for Count { fn new() -> Self { Count([0; 1])} }

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Reads<T> {
	pub general: T,
	pub unmapped: T,
	pub sequencing_control: Option<T>,
	pub under_conversion_control: Option<T>,
	pub over_conversion_control: Option<T>,
}

fn add_option<T: Add<Output = T>>(a: Option<T>, b: Option<T>) -> Option<T> 
{
	if let Some(y) = b	{
		if let Some(x) = a { Some(x + y) }
		else { Some(y) }
	} else { a }
}

impl<T: AddAssign + Copy + Add<Output = T>> Reads<T> {
	pub fn get_total(&self) -> T {
		let mut ct = self.general + self.unmapped;
		if let Some(x) = self.sequencing_control { ct += x }
		if let Some(x) = self.under_conversion_control { ct += x }
		if let Some(x) = self.over_conversion_control { ct += x }
		ct
	}	
}

impl<T: AddAssign + Copy + Add<Output = T>> AddAssign for Reads<T> {
    fn add_assign(&mut self, other: Self) {

		self.general += other.general; 
		self.unmapped += other.unmapped;
		self.sequencing_control = add_option(self.sequencing_control, other.sequencing_control);
		self.under_conversion_control = add_option(self.under_conversion_control, other.under_conversion_control);
		self.over_conversion_control = add_option(self.over_conversion_control, other.over_conversion_control);
	}
}

impl From<Reads<Count>> for Reads<Counts> {
	fn from(rd: Reads<Count>) -> Self {
		Self{ 
			general: rd.general.into(),
			unmapped: rd.unmapped.into(),
			sequencing_control: rd.sequencing_control.map(|c| c.into()),
			under_conversion_control: rd.under_conversion_control.map(|c| c.into()),
			over_conversion_control: rd.over_conversion_control.map(|c| c.into()),
		}
	}	
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct NumReadsBS<T> {
	pub c2t: T,
	pub g2a: T,
}

impl From<NumReadsBS<Count>> for NumReadsBS<Counts> {
	fn from(nr: NumReadsBS<Count>) -> Self {
		Self{ c2t: nr.c2t.into(), g2a: nr.g2a.into() }
	}	
}

impl<T: Add<Output = T>> Add for NumReadsBS<T> {
    type Output = Self;
    fn add(self, other: Self) -> Self { Self { c2t: self.c2t + other.c2t, g2a: self.g2a + other.g2a } }
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
impl<T: AddAssign> AddAssign for BaseCounts<T> {
    fn add_assign(&mut self, other: BaseCounts<T>) {
		self.a += other.a;
		self.c += other.c;
		self.g += other.g;
		self.t += other.t;
		self.n += other.n;
    }
}
impl<T: Add<Output = T>> Add for BaseCounts<T> {
    type Output = Self;
    fn add(self, other: Self) -> Self { 
		Self { a: self.a + other.a,	c: self.c + other.c,  g: self.g + other.g, t: self.t + other.t, n: self.n + other.n }
	}
}
impl<T: New> BaseCounts<T> {
	pub fn new() -> Self { Self{ a: T::new(), c: T::new(), g: T::new(), t: T::new(), n: T::new() }}
}
impl<T: Copy + Add<Output = T>> BaseCounts<T> {
	pub fn get_total(&self) -> T { self.a + self.c + self.g + self.t + self.n }	
}


impl From<Count> for Counts {
	fn from(c: Count) -> Self { Counts([c[0], 0]) }
}
impl From<BaseCounts<Count>> for BaseCounts<Counts> {
	fn from(bc: BaseCounts<Count>) -> Self {
		BaseCounts{ a: bc.a.into(), c: bc.c.into(), g: bc.g.into(), t: bc.t.into(), n: bc.n.into() }
	}	
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BaseCountStats<T> {
	pub overall: BaseCounts<T>,
	#[serde(rename = "GeneralC2T")]
	pub general_c2t: Option<BaseCounts<T>>,
	#[serde(rename = "GeneralG2A")]
	pub general_g2a: Option<BaseCounts<T>>,
	#[serde(rename = "UnderConversionControlC2T")]
	pub under_conversion_control_c2t: Option<BaseCounts<T>>,
	#[serde(rename = "UnderConversionControlG2A")]
	pub under_conversion_control_g2a: Option<BaseCounts<T>>,
	#[serde(rename = "OverConversionControlC2T")]
	pub over_conversion_control_c2t: Option<BaseCounts<T>>,
	#[serde(rename = "OverConversionControlG2A")]
	pub over_conversion_control_g2a: Option<BaseCounts<T>>,
}

impl<T: AddAssign + Copy + Add<Output = T>> AddAssign for BaseCountStats<T> {
    fn add_assign(&mut self, other: BaseCountStats<T>) {
		self.overall += other.overall;
		self.general_c2t = add_option(self.general_c2t, other.general_c2t);
		self.general_g2a = add_option(self.general_g2a, other.general_g2a);
		self.under_conversion_control_c2t = add_option(self.under_conversion_control_c2t, other.under_conversion_control_c2t);
		self.under_conversion_control_g2a = add_option(self.under_conversion_control_g2a, other.under_conversion_control_g2a);
		self.over_conversion_control_c2t = add_option(self.over_conversion_control_c2t, other.over_conversion_control_c2t);
		self.over_conversion_control_g2a = add_option(self.over_conversion_control_g2a, other.over_conversion_control_g2a);
    }
}

impl From<BaseCountStats<Count>> for BaseCountStats<Counts> {
	fn from(bc: BaseCountStats<Count>) -> Self {
		Self{
			overall: bc.overall.into(), 
			general_c2t: bc.general_c2t.map(|c| c.into()),
			general_g2a: bc.general_g2a.map(|c| c.into()),
			under_conversion_control_c2t: bc.under_conversion_control_c2t.map(|c| c.into()),
			under_conversion_control_g2a: bc.under_conversion_control_g2a.map(|c| c.into()),
			over_conversion_control_c2t: bc.over_conversion_control_c2t.map(|c| c.into()),
			over_conversion_control_g2a: bc.over_conversion_control_g2a.map(|c| c.into())
		}
	}	
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

impl Paired {
	fn merge(&mut self, other: &Self) {
		// Ignore read_groups
		self.reads += other.reads;
		self.num_reads_bs = add_option(self.num_reads_bs, other.num_reads_bs);
		self.correct_pairs += other.correct_pairs;
		self.base_counts += other.base_counts;
		add_assign_vec(&mut self.hist_mapq, &other.hist_mapq, 0);
		for i in 0..2 {
			for (key, ct) in other.hist_read_len[i].iter() { *(self.hist_read_len[i].entry(key.to_owned()).or_insert(0)) += ct; }
			for (key, ct) in other.hist_mismatch[i].iter() { *(self.hist_mismatch[i].entry(key.to_owned()).or_insert(0)) += ct; }
		}
		for (key, ct) in other.hist_template_len.iter() { *(self.hist_template_len.entry(key.to_owned()).or_insert(0)) += ct; }
	}
	pub fn reads(&self) -> Reads<Counts> { self.reads }
	pub fn bs_reads(&self) -> Option<NumReadsBS<Counts>> { self.num_reads_bs }
	pub fn base_counts(&self) -> BaseCountStats<Counts> { self.base_counts }
	pub fn correct_pairs(&self) -> usize { self.correct_pairs }
	pub fn read_len(&self) -> &[HashMap<String, usize>; 2] { &self.hist_read_len }
	pub fn mismatch(&self) -> &[HashMap<String, usize>; 2] { &self.hist_mismatch }
	pub fn template_len(&self) -> &HashMap<String, usize> { &self.hist_template_len }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Single {
	read_group: Option<String>,
	reads: Reads<Count>,
	#[serde(rename = "NumReadsBS")]
	num_reads_bs: Option<NumReadsBS<Count>>,
	base_counts: BaseCountStats<Count>,
	hist_mapq: Vec<usize>,
	hist_read_len: [HashMap<String, usize>; 1],
	hist_mismatch: [HashMap<String, usize>; 1],
}

impl Single {
	fn merge(&mut self, other: &Self) {
		// Ignore read_groups
		self.reads += other.reads;
		self.num_reads_bs = add_option(self.num_reads_bs, other.num_reads_bs);
		self.base_counts += other.base_counts;
		add_assign_vec(&mut self.hist_mapq, &other.hist_mapq, 0);
		for (key, ct) in other.hist_read_len[0].iter() { *(self.hist_read_len[0].entry(key.to_owned()).or_insert(0)) += ct; }
		for (key, ct) in other.hist_mismatch[0].iter() { *(self.hist_mismatch[0].entry(key.to_owned()).or_insert(0)) += ct; }
	}
	pub fn reads(&self) -> Reads<Count> { self.reads }
	pub fn bs_reads(&self) -> Option<NumReadsBS<Count>> { self.num_reads_bs }
	pub fn base_counts(&self) -> BaseCountStats<Count> { self.base_counts }
	pub fn read_len(&self) -> &HashMap<String, usize> { &self.hist_read_len[0] }
	pub fn mismatch(&self) -> &HashMap<String, usize> { &self.hist_mismatch[0] }
}

#[derive(Clone, Copy)]
enum MapJsonType { Paired, Unknown, Single }

#[derive(Clone, Deserialize)]
#[serde(tag = "MapperType", rename_all = "PascalCase")]
pub enum MapJson {
	Paired(Paired),
	Unknown(Paired),
	Single(Single),
} 

impl MapJson {
	fn get_type(&self) -> MapJsonType {
		match self {
			MapJson::Paired(_) => MapJsonType::Paired,
			MapJson::Single(_) => MapJsonType::Single,
			MapJson::Unknown(_) => MapJsonType::Unknown,
		}
	}
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
	pub fn get_mapq_hist(&self) -> &Vec<usize> {
		match self {
			MapJson::Paired(s) | MapJson::Unknown(s) => &s.hist_mapq,
			MapJson::Single(s) => &s.hist_mapq,
		}
	}
	pub fn get_unique(&self, thresh: usize) -> (usize, usize) {
		self.get_mapq_hist().iter().enumerate().fold((0, 0), |mut s, (q, x)| {
			if q >= thresh { s.0 += x }
			s.1 += x;
			s
		})		
	}
	pub fn merge(mut self, mut other: Self) -> Self {
		let t1 = self.get_type();
		let t2 = other.get_type();
		match (t1, t2) {
			(MapJsonType::Single, MapJsonType::Single) => self.merge_single(&other),
			(MapJsonType::Single, MapJsonType::Paired) | (MapJsonType::Single, MapJsonType::Unknown) | (MapJsonType::Paired, MapJsonType::Unknown) => {
				self = self.to_unknown();
				self.merge_paired_types(&other);
			},
			(MapJsonType::Paired, MapJsonType::Single) => {
				self = self.to_unknown();
				other = other.to_unknown();
				self.merge_paired_types(&other);
			},
			(MapJsonType::Unknown, MapJsonType::Single) => {
				other = other.to_unknown();
				self.merge_paired_types(&other);
			},
			_ => self.merge_paired_types(&other),
			
		}
		self		
	}
	fn merge_single(&mut self, other: &Self) {
		if let MapJson::Single(s1) = self {
			if let MapJson::Single(s2) = other {
				s1.merge(&s2);
			} else {panic!("Invalid conversion")}
		} else {panic!("Invalid conversion")}
	}
	
	fn merge_paired_types(&mut self, other: &Self) {
		if let MapJson::Paired(s1) | MapJson::Unknown(s1) = self {
			if let MapJson::Paired(s2) | MapJson::Unknown(s2) = other {
				s1.merge(&s2);
			} else {panic!("Invalid conversion")}
		} else {panic!("Invalid conversion")}
			
	}

	fn to_unknown(&self) -> Self {
		let t = self.get_type();
		let mjson = self.clone();
		match t {
			MapJsonType::Single => mjson.single_to_unknown(),
			MapJsonType::Paired => mjson.paired_to_unknown(),
			_ => mjson,
		}
	}
	fn single_to_unknown(self) -> Self {
		if let MapJson::Single(s) = self {
			let read_group = s.read_group;
			let reads: Reads<Counts> = s.reads.into();
			let num_reads_bs: Option<NumReadsBS<Counts>> = s.num_reads_bs.map(|nr| nr.into());
			let base_counts: BaseCountStats<Counts> = s.base_counts.into();  
			let hist_read_len = [s.hist_read_len[0].clone(), HashMap::new()];
			let hist_mismatch = [s.hist_mismatch[0].clone(), HashMap::new()];
			MapJson::Unknown(Paired{read_group, reads, num_reads_bs, correct_pairs: 0, 
				base_counts, hist_mapq: s.hist_mapq, hist_read_len, hist_mismatch, hist_template_len: HashMap::new()})
		} else { panic!("Invalid conversion"); }
	}
	fn paired_to_unknown(self) -> Self {
		if let MapJson::Paired(s) = self { MapJson::Unknown(s) } else { panic!("Invalid conversion") }
	} 
}

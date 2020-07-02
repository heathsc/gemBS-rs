use std::collections::HashMap;
use std::clone::Clone;
use std::ops::{AddAssign, Add};
use std::io::{Read, Write};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Counts {
	all: usize,
	passed: usize,
}

impl AddAssign for Counts {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            all: self.all + other.all,
            passed: self.passed + other.passed,
        };
    }
}

impl Add for Counts {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            all: self.all + other.all,
            passed: self.passed + other.passed,
        }
    }
}

impl Counts {
	fn new() -> Self { Self{all: 0, passed: 0} }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct QCCounts {
	non_variant: usize,
	variant: usize,
}

impl AddAssign for QCCounts {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            non_variant: self.non_variant + other.non_variant,
            variant: self.variant + other.variant,
        };
    }
}

impl QCCounts {
	fn new() -> Self { Self{non_variant: 0, variant: 0} }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MutCounts {
	all: usize,
	passed: usize,
	#[serde(rename = "dbSNPAll")]
	dbsnp_all: usize,
	#[serde(rename = "dbSNPPassed")]
	dbsnp_passed: usize,
}

impl AddAssign for MutCounts {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            all: self.all + other.all,
            passed: self.passed + other.passed,
			dbsnp_all: self.dbsnp_all + other.dbsnp_all,
			dbsnp_passed: self.dbsnp_passed + other.dbsnp_passed,
        };
    }
}

impl MutCounts {
	fn new() -> Self { Self{all: 0, passed: 0, dbsnp_all: 0, dbsnp_passed: 0} }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FSCounts {
	reads: usize,
	bases: usize,
}

impl AddAssign for FSCounts {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            reads: self.reads + other.reads,
            bases: self.bases + other.bases,
        };
    }
}

impl FSCounts {
	fn new() -> Self { Self{reads: 0, bases: 0} }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum FSReadLevelType { 
	Passed,
	Unmapped,
	#[serde(rename = "QC_Flags")]
	QCFlags,
	SecondaryAlignment,
	NoPosition,
	NoMatePosition,
	MisMatchContig,
	MateUnmapped,
	Duplicate,
	BadOrientation,
	LargeInsertSize,
	NoSequence,
	LowMAPQ,
	NotCorrectlyAligned,
	PairNotFound,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum FSBaseLevelType { 
	Passed,
	Trimmed,
	Clipped,
	Overlapping,
	LowQuality,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FSType { 
	read_level: HashMap<FSReadLevelType, FSCounts>,
	base_level: HashMap<FSBaseLevelType, usize>,
}

impl FSType {
	fn merge(&mut self, other: &Self) {
		// read_level
		for (key, ct) in other.read_level.iter() { *(self.read_level.entry(*key).or_insert_with(FSCounts::new)) += *ct; }
		// base level
		for (key, ct) in other.base_level.iter() { *(self.base_level.entry(*key).or_insert(0)) += ct; }
	}
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct QCDist {
	fisher_strand: HashMap<usize, usize>,	
	quality_by_depth: HashMap<usize, QCCounts>,
	#[serde(rename = "RMSMappingQuality")]
	rms_mapping_quality: HashMap<usize, QCCounts>,
}

impl QCDist {
	fn merge(&mut self, other: &Self) {
		// fisher_strand
		for (key, ct) in other.fisher_strand.iter() { *(self.fisher_strand.entry(*key).or_insert(0)) += ct; }
		// quality_by_depth
		for (key, ct) in other.quality_by_depth.iter() { *(self.quality_by_depth.entry(*key).or_insert_with(QCCounts::new)) += *ct; }
		// rms_mapping_quality
		for (key, ct) in other.rms_mapping_quality.iter() { *(self.rms_mapping_quality.entry(*key).or_insert_with(QCCounts::new)) += *ct; }
	}	
}

#[derive(Clone, Serialize, Deserialize)]
struct Coverage {
	#[serde(rename = "All")]
	all: HashMap<usize, usize>,	
	#[serde(rename = "Variant")]
	variant: HashMap<usize, usize>,	
	#[serde(rename = "RefCpG")]
	ref_cpg: HashMap<usize, usize>,	
	#[serde(rename = "RefCpGInf")]
	ref_cpg_inf: HashMap<usize, usize>,	
	#[serde(rename = "NonRefCpG")]
	non_ref_cpg: HashMap<usize, usize>,	
	#[serde(rename = "NonRefCpGInf")]
	non_ref_cpg_inf: HashMap<usize, usize>,	
	#[serde(rename = "GC")]
	gc: HashMap<usize, Vec<usize>>,	
}

impl Coverage {
	fn merge(&mut self, other: &Self) {
		
		// Standard fields are hashes of usize
		for (key, ct) in other.all.iter() { *(self.all.entry(*key).or_insert(0)) += ct; }
		for (key, ct) in other.variant.iter() { *(self.variant.entry(*key).or_insert(0)) += ct; }
		for (key, ct) in other.ref_cpg.iter() { *(self.ref_cpg.entry(*key).or_insert(0)) += ct; }
		for (key, ct) in other.ref_cpg_inf.iter() { *(self.ref_cpg_inf.entry(*key).or_insert(0)) += ct; }
		for (key, ct) in other.non_ref_cpg.iter() { *(self.non_ref_cpg.entry(*key).or_insert(0)) += ct; }
		for (key, ct) in other.non_ref_cpg_inf.iter() { *(self.non_ref_cpg_inf.entry(*key).or_insert(0)) += ct; }
		
		// GC is a hash of vectors
		for (key, ct) in other.gc.iter() { add_assign_vec(self.gc.entry(*key).or_insert_with(Vec::new), ct, 0); }
	}
}
fn add_assign_vec<T: Clone + Copy + AddAssign>(a: &mut Vec<T>, b: &[T], zero: T) {
	if b.len() > a.len() { a.resize(b.len(), zero); }
	for (i, x) in b.iter().enumerate() { a[i] += *x }
}

#[derive(Clone, Serialize, Deserialize)]
struct Quality {
	#[serde(rename = "All")]
	all: Vec<usize>,
	#[serde(rename = "Variant")]
	variant: Vec<usize>,
	#[serde(rename = "RefCpG")]
	ref_cpg: Vec<usize>,
	#[serde(rename = "NonRefCpG")]
	non_ref_cpg: Vec<usize>,
}

impl Quality {
	fn merge(&mut self, other: &Self) {
		add_assign_vec(&mut self.all, &other.all, 0);
		add_assign_vec(&mut self.variant, &other.variant, 0);
		add_assign_vec(&mut self.ref_cpg, &other.ref_cpg, 0);
		add_assign_vec(&mut self.non_ref_cpg, &other.non_ref_cpg, 0);
	}
} 

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Methylation {
	all_ref_cpg: Vec<f64>,
	passed_ref_cpg: Vec<f64>,
	all_non_ref_cpg: Vec<f64>,
	passed_non_ref_cpg: Vec<f64>,
	#[serde(rename = "NonCpGreadProfile")]
    #[serde(skip_serializing_if = "Option::is_none")]
	non_cpg_read_profile: Option<Vec<[usize; 4]>>,
}

impl Methylation {
	fn merge(&mut self, other: &Self) {
		// Vec<f64>
		add_assign_vec(&mut self.all_ref_cpg, &other.all_ref_cpg, 0.0);
		add_assign_vec(&mut self.passed_ref_cpg, &other.passed_ref_cpg, 0.0);
		add_assign_vec(&mut self.all_non_ref_cpg, &other.all_non_ref_cpg, 0.0);
		add_assign_vec(&mut self.passed_non_ref_cpg, &other.passed_non_ref_cpg, 0.0);
		
		// Non CpG Read Profile
		if let Some(b) = &other.non_cpg_read_profile {
			if let Some(a) = &mut self.non_cpg_read_profile {
				if b.len() > a.len() { a.resize(b.len(), [0, 0, 0, 0])}
				for (i, x) in b.iter().enumerate() { for (k, y) in x.iter().enumerate() { a[i][k] += y }} 
			} else {
				let mut a = Vec::new();
				for x in b.iter() { a.push(*x) }
				self.non_cpg_read_profile = Some(a);
			}
		}
	}
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct BasicStats { 
	#[serde(rename = "SNPS")]
	snps: Counts,
	#[serde(rename = "Indels")]
	indels: Counts,
	#[serde(rename = "Multiallelic")]
	multiallelic: Counts,
	#[serde(rename = "RefCpG")]
	ref_cpg: Counts,
	#[serde(rename = "NonRefCpG")]
	non_ref_cpg: Counts,	
}

impl BasicStats {
	fn new() -> Self {
		Self{ snps: Counts::new(), indels: Counts::new(), multiallelic: Counts::new(), ref_cpg: Counts::new(), non_ref_cpg: Counts::new() }
	}
}

impl AddAssign for BasicStats {
	fn add_assign(&mut self, other: Self) {
		self.snps += other.snps;
		self.indels += other.indels;
		self.multiallelic += other.multiallelic;
		self.ref_cpg += other.ref_cpg;
		self.non_ref_cpg += other.non_ref_cpg;
	}
}


#[derive(Clone, Copy, Serialize, Deserialize)]
struct CSType { 
	#[serde(flatten)]
	basic_stats: BasicStats,
	#[serde(rename = "dbSNPSites")]
    #[serde(skip_serializing_if = "Option::is_none")]
	dbsnp_sites: Option<Counts>,
	#[serde(rename = "dbSNPVariantSites")]
    #[serde(skip_serializing_if = "Option::is_none")]
	dbsnp_variants: Option<Counts>,
}

impl CSType {
	fn new() -> Self {
		Self{basic_stats: BasicStats::new(), dbsnp_sites: None, dbsnp_variants: None }
	}
}
impl AddAssign for CSType {
	fn add_assign(&mut self, other: Self) {
		let add_option_counts = |x, y| {
			if let Some(b) = y {
				if let Some(a) = x { Some(a + b) } else { y }
			} else { x }
		};
		// Basic Stats
		self.basic_stats += other.basic_stats;		
		self.dbsnp_sites = add_option_counts(self.dbsnp_sites, other.dbsnp_sites);
		self.dbsnp_variants = add_option_counts(self.dbsnp_variants, other.dbsnp_variants);		
	}
}

#[derive(Clone, Serialize, Deserialize)]
struct TSType { 
	#[serde(flatten)]
	basic_stats: BasicStats,
	#[serde(rename = "dbSNPSites")]
    #[serde(skip_serializing_if = "Option::is_none")]
	dbsnp_sites: Option<Counts>,
	#[serde(rename = "dbSNPVariants")]
    #[serde(skip_serializing_if = "Option::is_none")]
	dbsnp_variants: Option<Counts>,
	#[serde(rename = "QCDistributions")]
	qc_distributions: QCDist,
	#[serde(rename = "VCFFilterStats")]
	vcf_filter_stats: HashMap<String, QCCounts>,
	coverage: Coverage,
	quality: Quality,
	mutations: HashMap<String, MutCounts>,
	methylation: Methylation,
}

impl TSType {
	fn merge(&mut self, other: &Self) {
		let add_option_counts = |x, y| {
			if let Some(b) = y {
				if let Some(a) = x { Some(a + b) } else { y }
			} else { x }
		};
		// Basic Stats
		self.basic_stats += other.basic_stats;		
		self.dbsnp_sites = add_option_counts(self.dbsnp_sites, other.dbsnp_sites);
		self.dbsnp_variants = add_option_counts(self.dbsnp_variants, other.dbsnp_variants);		
		// Merge QC Distributions
		self.qc_distributions.merge(&other.qc_distributions);
		// Merge VCF Filter Stats
		for (key, ct) in other.vcf_filter_stats.iter() { *(self.vcf_filter_stats.entry(key.to_owned()).or_insert_with(QCCounts::new)) += *ct; }	
		// Merge Coverage
		self.coverage.merge(&other.coverage);
		// Merge Quality
		self.quality.merge(&other.quality);
		// Mutations
		for (key, ct) in other.mutations.iter() { *(self.mutations.entry(key.to_owned()).or_insert_with(MutCounts::new)) += *ct; }
		// Methylation
		self.methylation.merge(&other.methylation);
	}	
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallJson {
	source: String,
	date: String,
	filter_stats: FSType,
	contig_stats: HashMap<String, CSType>,
	total_stats: TSType,
}

impl CallJson {
	pub fn from_reader<T: Read>(rdr: T) -> Result<Self, String> {
		serde_json::from_reader(rdr).map_err(|e| format!("Couldn't parse call JSON file {}", e))
	}
	pub fn to_writer<T: Write>(&self, wrt: T) -> Result<(), String> {
		serde_json::to_writer(wrt, self).map_err(|e| format!("Error: failed to write JSON file: {}", e))		
	}
	pub fn merge(&mut self, other: &Self) {
		// We don't touch the source or date fields
		
		// Merge filter stats
		self.filter_stats.merge(&other.filter_stats);
		
		// Merge total stats
		self.total_stats.merge(&other.total_stats);
		
		// Merge contig stats
		for (ctg, ct) in other.contig_stats.iter() { *(self.contig_stats.entry(ctg.to_owned()).or_insert_with(CSType::new)) += *ct; }
	}
}

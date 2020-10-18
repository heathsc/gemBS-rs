use std::collections::HashMap;
use std::clone::Clone;
use std::ops::{AddAssign, Add, Sub};
use std::io::Write;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Counts {
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
	pub fn new() -> Self { Self{all: 0, passed: 0} }
	pub fn all(&self) -> usize { self.all }
	pub fn passed(&self) -> usize { self.passed } 
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QCCounts {
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

impl Add for QCCounts {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            non_variant: self.non_variant + other.non_variant,
            variant: self.variant + other.variant,
        }
    }
}

impl Sub for QCCounts {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            non_variant: self.non_variant.saturating_sub(other.non_variant),
            variant: self.variant.saturating_sub(other.variant),
        }
    }
}

impl QCCounts {
	pub fn new() -> Self { Self{non_variant: 0, variant: 0} }
	pub fn non_variant(&self) -> usize { self.non_variant }
	pub fn variant(&self) -> usize { self.variant }
	pub fn all(&self) -> usize {self.variant + self.non_variant}
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MutCounts {
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

impl Add for MutCounts {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            all: self.all + other.all,
            passed: self.passed + other.passed,
			dbsnp_all: self.dbsnp_all + other.dbsnp_all,
			dbsnp_passed: self.dbsnp_passed + other.dbsnp_passed,
        }
    }
}

impl MutCounts {
	pub fn new() -> Self { Self{all: 0, passed: 0, dbsnp_all: 0, dbsnp_passed: 0} }
	pub fn all(&self) -> usize {self.all}
	pub fn passed(&self) -> usize {self.passed}
	pub fn dbsnp_all(&self) -> usize {self.dbsnp_all}
	pub fn dbsnp_passed(&self) -> usize {self.dbsnp_passed}
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FSCounts {
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
	pub fn new() -> Self { Self{reads: 0, bases: 0} }
	pub fn reads(&self) -> usize { self.reads }
	pub fn bases(&self) -> usize { self.bases }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FSReadLevelType { 
	Passed,
	Unmapped,
	#[serde(rename = "QC_Flags")]
	QCFlags,
	SecondaryAlignment,
	SupplementaryAlignment,
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
	ZeroUnclipped,
}

impl FSReadLevelType {
    pub fn iter() -> impl Iterator<Item = (FSReadLevelType, &'static str)> {
        static GRAPHS: [(FSReadLevelType, &str); 15] = [
			(FSReadLevelType::Passed, "Passed"),
			(FSReadLevelType::LowMAPQ, "Low MAPQ"),
			(FSReadLevelType::NotCorrectlyAligned, "Not Correctly Aligned"),
			(FSReadLevelType::Unmapped, "Unmapped"),
			(FSReadLevelType::Duplicate, "Duplicate"),
			(FSReadLevelType::BadOrientation, "Bad Orientation"),
			(FSReadLevelType::LargeInsertSize, "Large Insert Size"),
			(FSReadLevelType::MisMatchContig, "Contigs Mismatched"),
			(FSReadLevelType::MateUnmapped, "Mate Unmapped"),
			(FSReadLevelType::QCFlags, "QC Flags"),
			(FSReadLevelType::SecondaryAlignment, "Secondary Alignment"),
			(FSReadLevelType::NoPosition, "No Position"),
			(FSReadLevelType::NoSequence, "No Sequence"),
			(FSReadLevelType::NoMatePosition, "No Mate Position"),
			(FSReadLevelType::PairNotFound, "PairNotFound"),
		];
        GRAPHS.iter().copied()
    }
}


#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FSBaseLevelType { 
	Passed,
	Trimmed,
	Clipped,
	Overlapping,
	LowQuality,
	Inserts,
}

impl FSBaseLevelType {
    pub fn iter() -> impl Iterator<Item = (FSBaseLevelType, &'static str)> {
        static GRAPHS: [(FSBaseLevelType, &str); 5] = [
			(FSBaseLevelType::Passed, "Passed"),
			(FSBaseLevelType::Overlapping, "Overlapping"),
			(FSBaseLevelType::LowQuality, "Low Quality"),
			(FSBaseLevelType::Trimmed, "Trimmed"),
			(FSBaseLevelType::Clipped, "Clipped"),
		];
        GRAPHS.iter().copied()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FSType { 
	read_level: HashMap<FSReadLevelType, FSCounts>,
	base_level: HashMap<FSBaseLevelType, usize>,
}

impl FSType {
	pub fn add_read_level_count(&mut self, fs_type: FSReadLevelType, counts: usize) {
		let mut fc = self.read_level.entry(fs_type).or_insert_with(FSCounts::new);
		fc.reads += 1;
		fc.bases += counts;
	}
	pub fn add_read_level_fs_counts(&mut self, fs_type: FSReadLevelType, counts: FSCounts) {
		let fc = self.read_level.entry(fs_type).or_insert_with(FSCounts::new);
		*fc += counts;
	}
	pub fn add_base_level_count(&mut self, fs_type: FSBaseLevelType, bases: usize) {
		let fc = self.base_level.entry(fs_type).or_insert(0);
		*fc += bases;
	}
	pub fn read_level(&self) -> &HashMap<FSReadLevelType, FSCounts> { &self.read_level }
	pub fn base_level(&self) -> &HashMap<FSBaseLevelType, usize> { &self.base_level }
}
impl FSType {
	pub fn new() -> Self { Self{read_level: HashMap::new(), base_level: HashMap::new()}}
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QCDist {
	pub fisher_strand: HashMap<usize, usize>,	
	pub quality_by_depth: HashMap<usize, QCCounts>,
	#[serde(rename = "RMSMappingQuality")]
	pub rms_mapping_quality: HashMap<usize, QCCounts>,
}

impl QCDist {
	fn new() -> Self { 
		Self{ 
			fisher_strand: HashMap::new(),
			quality_by_depth: HashMap::new(),
			rms_mapping_quality: HashMap::new(),
		}
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Coverage {
	#[serde(rename = "All")]
	pub all: HashMap<usize, usize>,	
	#[serde(rename = "Variant")]
	pub variant: HashMap<usize, usize>,	
	#[serde(rename = "RefCpG")]
	pub ref_cpg: HashMap<usize, usize>,	
	#[serde(rename = "RefCpGInf")]
	pub ref_cpg_inf: HashMap<usize, usize>,	
	#[serde(rename = "NonRefCpG")]
	pub non_ref_cpg: HashMap<usize, usize>,	
	#[serde(rename = "NonRefCpGInf")]
	pub non_ref_cpg_inf: HashMap<usize, usize>,	
	#[serde(rename = "GC")]
	pub gc: HashMap<usize, Vec<usize>>,	
}

impl Coverage {
	fn new() -> Self { 
		Self{ 
			all: HashMap::new(),
			variant: HashMap::new(),
			ref_cpg: HashMap::new(),
			ref_cpg_inf: HashMap::new(),
			non_ref_cpg: HashMap::new(),
			non_ref_cpg_inf: HashMap::new(),
			gc: HashMap::new()
		}
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Quality {
	#[serde(rename = "All")]
	pub all: Vec<usize>,
	#[serde(rename = "Variant")]
	pub variant: Vec<usize>,
	#[serde(rename = "RefCpG")]
	pub ref_cpg: Vec<usize>,
	#[serde(rename = "NonRefCpG")]
	pub non_ref_cpg: Vec<usize>,
}

impl Quality {
	fn new() -> Self { 
		Self{ 
			all: Vec::new(),
			variant: Vec::new(),
			ref_cpg: Vec::new(),
			non_ref_cpg: Vec::new(),
		}
	}
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Methylation {
	pub all_ref_cpg: Vec<f64>,
	pub passed_ref_cpg: Vec<f64>,
	pub all_non_ref_cpg: Vec<f64>,
	pub passed_non_ref_cpg: Vec<f64>,
	#[serde(rename = "NonCpGreadProfile")]
    #[serde(skip_serializing_if = "Option::is_none")]
	pub non_cpg_read_profile: Option<Vec<[usize; 4]>>,
}

impl Methylation {
	fn new() -> Self { 
		Self{ 
			all_ref_cpg: Vec::new(),
			passed_ref_cpg: Vec::new(),
			all_non_ref_cpg: Vec::new(),
			passed_non_ref_cpg: Vec::new(),
			non_cpg_read_profile: None,
		}
	}
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct BasicStats { 
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
	pub fn snps(&self) -> &Counts { &self.snps }
	pub fn indels(&self) -> &Counts { &self.indels }
	pub fn multiallelic(&self) -> &Counts { &self.multiallelic }
	pub fn ref_cpg(&self) -> &Counts { &self.ref_cpg }
	pub fn non_ref_cpg(&self) -> &Counts { &self.non_ref_cpg }
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

#[derive(Clone, Serialize, Deserialize)]
pub struct TSType { 
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
	fn new() -> Self { 
		Self{
			basic_stats: BasicStats::new(),
			dbsnp_sites: None, dbsnp_variants: None,
			qc_distributions: QCDist::new(),
			vcf_filter_stats: HashMap::new(),
			coverage: Coverage::new(),
			quality: Quality::new(),
			mutations: HashMap::new(),
			methylation: Methylation::new()
		}
	}
	pub fn methylation(&mut self) -> &mut Methylation { &mut self.methylation }
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
	pub fn new<S: AsRef<str>, T: AsRef<str>>(source: S, date: T) -> Self {
		Self {
			source: source.as_ref().to_owned(),
			date: date.as_ref().to_owned(),
			filter_stats: FSType::new(),
			contig_stats: HashMap::new(),
			total_stats: TSType::new(),
		}
	}
	pub fn to_writer<T: Write>(&self, wrt: T) -> Result<(), String> {
		serde_json::to_writer_pretty(wrt, self).map_err(|e| format!("Error: failed to write JSON file: {}", e))		
	}
	pub fn coverage(&self) -> &Coverage { &self.total_stats.coverage }
	pub fn quality(&self) -> &Quality { &self.total_stats.quality }
	pub fn qc_dist(&self) -> &QCDist { &self.total_stats.qc_distributions }
	pub fn methylation(&self) -> &Methylation { &self.total_stats.methylation }
	pub fn filter_stats(&mut self) -> &mut FSType { &mut self.filter_stats}
	pub fn total_stats(&mut self) -> &mut TSType { &mut self.total_stats}
	pub fn basic_stats(&self) -> &BasicStats { &self.total_stats.basic_stats }
	pub fn vcf_filter_stats(&self) -> &HashMap<String, QCCounts> { &self.total_stats.vcf_filter_stats }
	pub fn mutations(&self) -> &HashMap<String, MutCounts> { &self.total_stats.mutations }
}
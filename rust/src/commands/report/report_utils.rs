use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc, Mutex, RwLock};
use std::thread;
use std::collections::HashMap;

use crate::common::json_call_stats::CallJson;
use crate::common::latex_utils::LatexDoc;

#[derive(PartialEq, Eq, Clone)]
pub enum JobStatus {
	Ready,
	Running,
	Completed,
}

#[derive(Clone)]
pub struct DatasetJob {
	pub dataset: String,
	pub json_path: PathBuf,
	pub mapq_threshold: usize,
}

impl DatasetJob {
	pub fn new(dataset: &str, json_path: &Path, mapq_threshold: usize) -> Self {
		DatasetJob{dataset: dataset.to_owned(), json_path: json_path.to_owned(), mapq_threshold }
	}
}

#[derive(Clone)]
pub struct SampleJob {
	pub datasets: Vec<(String, PathBuf)>,
	pub depend: Vec<usize>,
	pub summary: Arc<Mutex<Vec<SampleSummary>>>,
	pub latex_doc: Arc<Mutex<LatexDoc>>,
	pub mapq_threshold: usize,
}

impl SampleJob {
	pub fn new(summary: Arc<Mutex<Vec<SampleSummary>>>, latex_doc: Arc<Mutex<LatexDoc>>, mapq_threshold:usize) -> Self {
		SampleJob{datasets: Vec::new(), depend: Vec::new(), summary, mapq_threshold, latex_doc}
	}
	pub fn add_dataset(&mut self, dataset: &str, path: &Path) -> &mut Self {
		self.datasets.push((dataset.to_owned(), path.to_owned()));
		self
	}
}

#[derive(Clone)]
pub struct LoadCallJson {
	pub path: PathBuf,
	pub call_json: Arc<RwLock<Option<CallJson>>>,	
}

#[derive(Debug, Clone, Copy)]
pub enum CallJob {
	CoverageAll,
	CoverageNonRefCpg,
	CoverageNonRefCpgInf,
	CoverageRefCpg,
	CoverageRefCpgInf,
	CoverageVariant,
	FsVariants,
	GCCoverage,
	MethylationLevels,
	NonCpgReadProfile,
	QdNonVariant,
	QdVariant,
	QualityAll,
	QualityRefCpg,
	QualityNonRefCpg,
	QualityVariant,
	RmsMqVariant,
	RmsMqNonVariant,
	MappingReport,
	MethylationReport,
	VariantReport,
}

impl CallJob {
    pub fn iter() -> impl Iterator<Item = CallJob> {
        static GRAPHS: [CallJob; 21] = [
			CallJob::CoverageAll,
			CallJob::CoverageNonRefCpg,
			CallJob::CoverageNonRefCpgInf,
			CallJob::CoverageRefCpg,
			CallJob::CoverageRefCpgInf,
			CallJob::CoverageVariant,
			CallJob::FsVariants,
			CallJob::GCCoverage,
			CallJob::MethylationLevels,
			CallJob::NonCpgReadProfile,
			CallJob::QdNonVariant,
			CallJob::QdVariant,
			CallJob::QualityAll,
			CallJob::QualityRefCpg,
			CallJob::QualityNonRefCpg,
			CallJob::QualityVariant,
			CallJob::RmsMqVariant,
			CallJob::RmsMqNonVariant,
			CallJob::MappingReport,
			CallJob::MethylationReport,
			CallJob::VariantReport,
		];
        GRAPHS.iter().copied()
    }
}

#[derive(Clone)]
pub struct MakeCallJob {
	pub job_type: CallJob,
	pub depend: usize,
	pub call_json: Arc<RwLock<Option<CallJson>>>,	
	pub summary: Arc<Mutex<HashMap<String, CallSummary>>>,

}

#[derive(Clone)]
pub enum RepJob {
	Dataset(DatasetJob),
	Sample(SampleJob),
	CallJson(LoadCallJson),	
	CallJob(MakeCallJob),
}


#[derive(Clone)]
pub struct ReportJob {
	pub barcode: String,
	pub bc_dir: PathBuf,
	pub ix: usize,
	pub status: JobStatus,
	pub project: String,
	pub job: RepJob, 	
}

impl ReportJob {
	pub fn new(bc: &str, project: &str, bc_dir: &Path, job: RepJob) -> Self {
		ReportJob{barcode: bc.to_owned(), bc_dir: bc_dir.to_owned(), project: project.to_owned(), job, status: JobStatus::Ready, ix: 0}
	}
}

pub struct Worker {
	pub handle: thread::JoinHandle<Result<(), String>>,
	pub tx: mpsc::Sender<Option<ReportJob>>,
	pub ix: usize,
}

pub struct SampleSummary {
	pub barcode: String,
	pub reads: usize,
	pub fragments: usize,
	pub unique: usize,
	pub conversion: Option<f64>,
	pub overconversion: Option<f64>,
}

#[derive(Debug)]
pub struct MapSummary {
	pub aligned: usize,
	pub unique: usize,
	pub passed: usize,
	pub gc_correlation: f64,	
}

impl MapSummary {
	pub fn new() -> Self { MapSummary{aligned: 0, unique: 0, passed: 0, gc_correlation: 0.0} }
}

#[derive(Debug)]
pub struct VarSummary {
	pub variants: usize,
	pub variants_passed: usize,
	pub med_cov_var_passed: usize,
	pub ti_tv_ratio: f64,		
}

impl VarSummary {
	pub fn new() -> Self { VarSummary{variants: 0, variants_passed: 0, med_cov_var_passed: 0, ti_tv_ratio: 0.0}}
}

#[derive(Debug)]
pub struct MethSummary {
	pub med_cpg_meth: f64,
	pub med_cpg_cov: usize,
	pub passed_cpgs: usize,	
}

impl MethSummary {
	pub fn new() -> Self {MethSummary{med_cpg_meth: 0.0, med_cpg_cov: 0, passed_cpgs: 0}}
}

#[derive(Debug)]
pub struct CallSummary {
	pub map: Option<MapSummary>,
	pub var: Option<VarSummary>,
	pub meth: Option<MethSummary>,
}

impl CallSummary {
	pub fn new() -> Self { CallSummary{map: None, var: None, meth: None }}
}

pub fn pct(a: usize, b: usize) -> f64 {
	if b > 0 { 100.0 * (a as f64) / (b as f64) }
	else { 0.0 }	
}


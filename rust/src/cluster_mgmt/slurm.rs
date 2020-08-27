use std::path::Path;
use std::fs;
use std::str::FromStr;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write, Seek, SeekFrom};
use std::rc::Rc;

use regex::Regex;
use lazy_static::lazy_static;

use crate::config::GemBS;
use crate::common::defs::{DataValue, JobLen, MemSize, Command};
use crate::common::dry_run;
use crate::common::utils::Pipeline;
use crate::common::tasks::TaskList;
use crate::cli::utils::LogLevel;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct SlurmDep {
	job_ix: usize,
	task_ix: usize,
}

#[derive(PartialEq, Eq, Hash)]
struct JobNode {
	cores: usize,
	mem: MemSize,
	time: JobLen,
	depend: Vec<SlurmDep>, // Index in vector of SlurmJobs	
}

struct SlurmJob {
	task_vec: Vec<usize>, // Index in task_list
	node: Rc<JobNode>,
}

impl SlurmJob {
	fn new(node: Rc<JobNode>) -> Self {
		SlurmJob{task_vec: Vec::new(), node}
	}
}

fn write_sbatch_script(mut file: &fs::File, jv: &SlurmJob, tl: &TaskList,  options: &HashMap<&'static str, DataValue>, verbose: LogLevel) -> std::io::Result<()> {
	writeln!(file, "#!/bin/sh")?;
	let job_array = jv.task_vec.len() > 1;
	if job_array {
	writeln!(file, "coms=( \\")?;
	for ix in jv.task_vec.iter() {
			let task = &tl[*ix];
			writeln!(file,"\"{} {}\" \\",task.command(), dry_run::get_arg_string(task, options))?;			
		}
		writeln!(file, ")")?;
		writeln!(file, "echo gemBS --loglevel {} ${{coms[$SLURM_ARRAY_TASK_ID]}}", verbose)?;
		writeln!(file, "gemBS --loglevel {} ${{coms[$SLURM_ARRAY_TASK_ID]}}", verbose)?;
	} else {
		let task = &tl[jv.task_vec[0]];
		writeln!(file,"gemBS {} {}",task.command(), dry_run::get_arg_string(task, options))?;			
	}
	Ok(())
}

fn write_sbatch_rm_script(mut file: &fs::File, logfiles: &[String]) -> std::io::Result<()> {
	writeln!(file, "#!/bin/sh")?;
	write!(file, "for f in")?;
	for f in logfiles.iter() {
		write!(file," \\\n {}", f)?;
	}
	writeln!(file,"\ndo\n rm -f slurm_logs/slurm_gemBS-${{f}}.out\ndone\necho Pipeline terminated successfully")
}

fn get_slurm_id(mut file: fs::File) -> Result<usize, String> {
	lazy_static! {
        static ref RE: Regex = Regex::new(r"^Submitted batch job (\d+)").unwrap();
	}
	file.seek(SeekFrom::Start(0)).map_err(|e| format!("{}", e))?;
	let mut content = String::new();
       file.read_to_string(&mut content).unwrap();
	if let Some(cap) = RE.captures(content.as_str()) {
		if let Ok(x) = <usize>::from_str(cap.get(1).unwrap().as_str()) { return Ok(x); }
	}
	Err(format!("Could not parse output from sbatch: {}", content))
}

// Prepare job graph and submit to slurm
pub fn handle_slurm(gem_bs: &GemBS, options: &HashMap<&'static str, DataValue>, task_list: &[usize]) -> Result<(), String> {
	let _ = fs::create_dir("slurm_logs");
	let mut job_vec: Vec<SlurmJob> = Vec::new();
	let mut slurm_id = Vec::new();
	let mut job_hash: HashMap<Rc<JobNode>, usize> = HashMap::new();
	let mut task_hash: HashMap<usize, SlurmDep> = HashMap::new();
	for ix in task_list.iter().filter(|i| gem_bs.get_tasks()[**i].command() != Command::MergeCallJsons) {
		let task = &gem_bs.get_tasks()[*ix];
		let depend = {
			let mut t = Vec::new();
			for i in task.parents().iter() {
				if let Some(x) = task_hash.get(i) { t.push(*x); }
			}
			t
		};
		let cores = task.cores().unwrap_or(1);
		let mem = task.memory().unwrap_or_else(|| MemSize::from(0x400000000)); // 1G
		let time = task.time().unwrap_or_else(|| JobLen::from(3600)); // 1hr
		let node = JobNode{cores, mem, time, depend};
		let job_ix = if let Some(i) = job_hash.get(&node) {
			job_vec[*i].task_vec.push(*ix);
			SlurmDep{job_ix: *i, task_ix: job_vec[*i].task_vec.len() - 1}
		} else {
			let node_rc = Rc::new(node);
			let mut job = SlurmJob::new(node_rc.clone());
			job.task_vec.push(*ix);
			let x = job_vec.len();
			job_vec.push(job);
			job_hash.insert(node_rc.clone(), x);
			SlurmDep{job_ix: x, task_ix: 0}
		};
		task_hash.insert(*ix, job_ix);
	}
	let verbose = gem_bs.verbose();
	let sbatch_path = Path::new("sbatch");
	let mut dep_hash = HashSet::new();
	for jv in job_vec.iter() {
		let mut tfile = tempfile::tempfile().expect("Couldn't create temporary slurm input file");
		write_sbatch_script(&tfile, jv, gem_bs.get_tasks(), options, verbose).map_err(|e| format!("Error writing sbatch script: {}", e))?;
		tfile.seek(SeekFrom::Start(0)).map_err(|e| format!("{}", e))?;
		let ofile = tempfile::tempfile().expect("Couldn't create temporary slurm output file");
		let mut sbatch_args = Vec::new();
		let mut hs = HashSet::new();
		let mut desc = String::from("gemBS");
		for ix in jv.task_vec.iter() {
			let task = &gem_bs.get_tasks()[*ix];
			if hs.insert(task.command()) {
				desc.push_str(format!("_{:#}",task.command()).as_str());
			}
		}
		sbatch_args.push(format!("--job-name={}", desc));
		sbatch_args.push(format!("--cpus-per-task={}", jv.node.cores));
		sbatch_args.push(format!("--mem={:#}", jv.node.mem));
		sbatch_args.push(format!("--time={}", jv.node.time));
		if jv.task_vec.len() > 1 { 
			sbatch_args.push(format!("--array=0-{}", jv.task_vec.len() - 1)); 
			sbatch_args.push("--output=slurm_logs/slurm_gemBS-%A_%a.out".to_string());
		} else { sbatch_args.push("--output=slurm_logs/slurm_gemBS-%j.out".to_string()); }
		if !jv.node.depend.is_empty() {
			let mut t = String::from("--dependency=afterok");
			for ix in jv.node.depend.iter() {
				let jv1 = &job_vec[ix.job_ix];
				let slurm_job_id = slurm_id[ix.job_ix];
				let job_id = if jv1.task_vec.len() > 1 {
					format!(":{}_{}", slurm_job_id, ix.task_ix)
				} else {
					format!(":{}", slurm_job_id)
				};
				t.push_str(job_id.as_str());
				dep_hash.insert(ix);
			}
			sbatch_args.push(t);
		}
		let mut pipeline = Pipeline::new();		
		pipeline.add_stage(&sbatch_path, Some(sbatch_args.iter()))
				.in_file(tfile)
				.out_file(ofile.try_clone().expect("Couldn't clone output file descriptor"));
		pipeline.run(gem_bs.get_signal_clone())?;
		slurm_id.push(get_slurm_id(ofile)?);
	}
	let mut deps = String::new();
	let mut logfiles = Vec::new();
	for (ix, jv) in job_vec.iter().enumerate() {
		let len = jv.task_vec.len();
		let job_id = slurm_id[ix];
		if len == 1 {
			let sdep = SlurmDep{job_ix: ix, task_ix: 0};
			if ! dep_hash.contains(&sdep) { deps.push_str(format!(":{}", job_id).as_str()); }
			logfiles.push(format!("{}", job_id));
		} else {
			for i in 0..len {
				let sdep = SlurmDep{job_ix: ix, task_ix: i};
				if ! dep_hash.contains(&sdep) { deps.push_str(format!(":{}_{}", job_id, i).as_str()); }
				logfiles.push(format!("{}_{}", job_id, i));
			}
		}
	}
	if ! logfiles.is_empty() {
		let mut tfile = tempfile::tempfile().expect("Couldn't create temporary slurm input file");
		write_sbatch_rm_script(&tfile, &logfiles).map_err(|e| format!("Error writing sbatch script: {}", e))?;
		tfile.seek(SeekFrom::Start(0)).map_err(|e| format!("{}", e))?;
		let mut sbatch_args = vec!("--job-name=gemBS_clean_logfiles", "--cpus-per-task=1", "--time=10", "--output=slurm_logs/slurm_gemBS_pipeline.out");
		if ! deps.is_empty() { 
			deps = format!("--dependency=afterok{}", deps); 
			sbatch_args.push(deps.as_str());
		}
		let ofile = tempfile::tempfile().expect("Couldn't create temporary slurm output file");
		let mut pipeline = Pipeline::new();		
		pipeline.add_stage(&sbatch_path, Some(sbatch_args.iter()))
				.in_file(tfile)
				.out_file(ofile.try_clone().expect("Couldn't clone output file descriptor"));
		pipeline.run(gem_bs.get_signal_clone())?;
		let _ = get_slurm_id(ofile)?;	
	}
	Ok(())
}

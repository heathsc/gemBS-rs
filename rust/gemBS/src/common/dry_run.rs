#[cfg(feature = "slurm")]
use crate::cluster_mgmt::slurm;
use crate::common::assets::GetAsset;
use crate::common::defs::{Command, DataValue};
use crate::common::tasks::{JsonTask, Task};
use crate::config::GemBS;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::BufWriter;

use std::path::Path;

pub fn get_arg_string(task: &Task, options: &HashMap<&'static str, (DataValue, bool)>) -> String {
    let mut arg_string = task.args().to_owned();
    for (opt, val) in options {
        assert!(!(*opt).starts_with('_'), "Old style option found");
        let option = opt.replace("_", "-");
        let s = match val {
            (DataValue::Int(x), false) => format!(" --{} {}", option, x),
            (DataValue::Float(x), false) => format!(" --{} {}", option, x),
            (DataValue::String(x), false) => format!(" --{} {}", option, x),
            (DataValue::FileType(x), false) => format!(" --{} {}", option, x),
            (DataValue::Bool(_), false) => format!(" --{}", option),
            (DataValue::StringVec(v), false) => {
                v.iter().fold(format!(" --{}", option), |mut s, x| {
                    s.push_str(format!(" {}", x).as_str());
                    s
                })
            }
            (DataValue::FloatVec(v), false) => {
                v.iter().fold(format!(" --{}", option), |mut s, x| {
                    s.push_str(format!(" {}", x).as_str());
                    s
                })
            }
            _ => String::new(),
        };
        if !s.is_empty() {
            arg_string.push_str(&s);
        }
    }
    arg_string
}

fn handle_dry_run(
    gem_bs: &GemBS,
    options: &HashMap<&'static str, (DataValue, bool)>,
    task_list: &[usize],
) {
    for ix in task_list {
        let task = &gem_bs.get_tasks()[*ix];
        if task.command() != Command::MergeCallJsons {
            let arg_string = get_arg_string(task, options);
            println!("gemBS {} {}", task.command(), arg_string);
        }
    }
}

fn handle_json_tasks(
    gem_bs: &GemBS,
    options: &HashMap<&'static str, (DataValue, bool)>,
    task_list: &[usize],
    json_file: &str,
) -> Result<(), String> {
    let task_set: HashSet<usize> = task_list.iter().fold(HashSet::new(), |mut hs, x| {
        hs.insert(*x);
        hs
    });
    let mut json_task_list = Vec::new();
    for ix in task_list {
        let task = &gem_bs.get_tasks()[*ix];
        if task.command() != Command::MergeCallJsons {
            let args = get_arg_string(task, options);
            let id = task.id();
            let command = format!("{}", task.command());
            let inputs: Vec<&Path> = task
                .inputs()
                .map(|x| gem_bs.get_asset(*x).unwrap().path())
                .collect();
            let outputs: Vec<&Path> = task
                .outputs()
                .map(|x| gem_bs.get_asset(*x).unwrap().path())
                .collect();
            let depend: Vec<&str> = task
                .parents()
                .iter()
                .filter(|x| task_set.contains(x))
                .map(|x| gem_bs.get_tasks()[*x].id())
                .collect();
            let mut jtask = JsonTask::new(
                id,
                command,
                args,
                inputs,
                outputs,
                depend,
                task.status().unwrap(),
            );
            if let Some(x) = task.cores() {
                jtask.add_cores(x);
            }
            if let Some(x) = task.memory() {
                jtask.add_memory(x);
            }
            if let Some(x) = task.time() {
                jtask.add_time(x);
            }
            json_task_list.push(jtask);
        }
    }
    let ofile = match fs::File::create(Path::new(json_file)) {
        Err(e) => return Err(format!("Couldn't open {}: {}", json_file, e)),
        Ok(f) => f,
    };
    let writer = BufWriter::new(ofile);
    serde_json::to_writer_pretty(writer, &json_task_list).map_err(|e| {
        format!(
            "Error: failed to write JSON config file {}: {}",
            json_file, e
        )
    })
}

pub fn handle_nonexec(
    gem_bs: &GemBS,
    options: &HashMap<&'static str, (DataValue, bool)>,
    task_list: &[usize],
) -> Result<(), String> {
    if gem_bs.dry_run() {
        handle_dry_run(gem_bs, &options, &task_list)
    }
    if let Some(json_file) = gem_bs.json_out() {
        handle_json_tasks(gem_bs, &options, &task_list, json_file)?;
    }

    #[cfg(feature = "slurm")]
    if gem_bs.slurm() || gem_bs.slurm_script().is_some() {
        slurm::handle_slurm(gem_bs, &options, &task_list)?;
    }

    Ok(())
}

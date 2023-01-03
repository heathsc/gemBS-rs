use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use super::{QPipe, QPipeCom};
use crate::common::assets::GetAsset;
use crate::common::defs::{Command, DataValue, Metadata, Section};
use crate::common::json_call_stats::CallJson;
use crate::common::latex_utils::PageSize;
use crate::common::utils::check_signal;
use crate::config::GemBS;
use utils::compress;

#[derive(Debug)]
pub struct SampleJsonFiles {
    pub barcode: String,
    pub bc_dir: PathBuf,
    pub json_files: Vec<(String, PathBuf)>,
}

#[derive(Debug)]
pub struct CallJsonFiles {
    pub barcode: String,
    pub bc_dir: PathBuf,
    pub json_file: PathBuf,
}

#[derive(Debug)]
pub struct MergeJsonFiles {
    pub barcode: String,
    pub json_files: Vec<(String, PathBuf)>,
}

pub fn make_map_report_pipeline(gem_bs: &GemBS, job: usize) -> QPipe {
    let task = &gem_bs.get_tasks()[job];
    let (nc, _) = super::get_command_req(gem_bs, Command::MapReport);

    let n_cores = (nc.round() as usize).max(1);
    let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
    let project = gem_bs
        .get_config_str(Section::Report, "project")
        .map(|x| x.to_owned());
    let mapq_thresh = gem_bs
        .get_config_int(Section::Report, "mapq_threshold")
        .unwrap_or(20);
    for out in task.outputs() {
        pipeline.add_outputs(
            gem_bs
                .get_asset(*out)
                .expect("Couldn't get map-report output asset")
                .path(),
        );
    }
    let href = gem_bs.get_sample_data_ref();
    let mut bcodes = HashMap::new();
    for (dataset, href1) in href.iter() {
        if let Some(DataValue::String(bc)) = href1.get(&Metadata::SampleBarcode) {
            bcodes
                .entry(bc.to_owned())
                .or_insert_with(Vec::new)
                .push(dataset.to_owned());
        } else {
            panic!("No barcode associated with dataset {}", dataset);
        }
    }
    let mut json_files = Vec::new();
    for (bc, dvec) in bcodes.iter() {
        let bc_dir = gem_bs
            .get_asset(format!("{}_map_index.html", bc).as_str())
            .expect("Culdn't find map report asset")
            .path()
            .parent()
            .expect("No parent dir found for map report file")
            .to_owned();
        let v = if dvec.len() == 1 {
            let json = gem_bs
                .get_asset(format!("{}_map.json", bc).as_str())
                .expect("Culdn't find JSON map asset")
                .path();
            let dat = dvec[0].to_owned();
            vec![(dat, json.to_owned())]
        } else {
            let mut v = Vec::new();
            for dat in dvec.iter() {
                let json = gem_bs
                    .get_asset(format!("{}_map.json", dat).as_str())
                    .expect("Culdn't find JSON map asset")
                    .path();
                v.push((dat.to_owned(), json.to_owned()))
            }
            v
        };
        json_files.push(SampleJsonFiles {
            barcode: bc.to_owned(),
            bc_dir,
            json_files: v,
        });
    }
    let mut css_dir = gem_bs.get_css_path();
    css_dir.push("style.css");
    let com = QPipeCom::MapReport((project, css_dir, mapq_thresh as usize, n_cores, json_files));
    pipeline.add_com(com);
    pipeline
}

pub fn make_call_report_pipeline(gem_bs: &GemBS, job: usize) -> QPipe {
    let task = &gem_bs.get_tasks()[job];
    let (nc, _) = super::get_command_req(gem_bs, Command::MapReport);
    let n_cores = {
        let x = (nc + 0.5) as usize;
        if x < 1 {
            1
        } else {
            x
        }
    };
    let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
    let project = gem_bs
        .get_config_str(Section::Report, "project")
        .map(|x| x.to_owned());
    for out in task.outputs() {
        pipeline.add_outputs(
            gem_bs
                .get_asset(*out)
                .expect("Couldn't get call-report output asset")
                .path(),
        );
    }
    let mut json_files = Vec::new();
    let samples = gem_bs.get_samples();
    for (bc, _) in samples.iter() {
        let bc_dir = gem_bs
            .get_asset(format!("{}_mapping_coverage.html", bc).as_str())
            .expect("Couldn't find call report asset")
            .path()
            .parent()
            .expect("No parent dir found for call report file")
            .to_owned();
        let json_file = gem_bs
            .get_asset(format!("{}_call.json", bc).as_str())
            .expect("Couldn't find call JSON asset for call report")
            .path()
            .to_owned();
        json_files.push(CallJsonFiles {
            barcode: bc.to_owned(),
            bc_dir,
            json_file,
        });
    }
    let mut css_dir = gem_bs.get_css_path();
    css_dir.push("style.css");
    let com = QPipeCom::CallReport((project, css_dir, n_cores, json_files));
    pipeline.add_com(com);
    pipeline
}

pub fn make_report_pipeline(gem_bs: &GemBS, job: usize) -> QPipe {
    let task = &gem_bs.get_tasks()[job];
    let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
    for out in task.outputs() {
        pipeline.add_outputs(
            gem_bs
                .get_asset(*out)
                .expect("Couldn't get report output asset")
                .path(),
        );
    }
    let project = gem_bs
        .get_config_str(Section::Report, "project")
        .map(|x| x.to_owned());
    let page_size =
        if let Some(DataValue::PageSize(s)) = gem_bs.get_config(Section::Report, "paper_size") {
            *s
        } else {
            PageSize::A4
        };
    let pdf = gem_bs.get_config_bool(Section::Report, "pdf");
    let com = QPipeCom::Report((project, page_size, pdf));
    pipeline.add_com(com);
    pipeline
}

pub fn make_merge_call_jsons_pipeline(gem_bs: &GemBS, job: usize) -> QPipe {
    let task = &gem_bs.get_tasks()[job];
    let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
    for out in task.outputs() {
        pipeline.add_outputs(
            gem_bs
                .get_asset(*out)
                .expect("Couldn't get merge-call-jsons output asset")
                .path(),
        );
    }
    let bc = task
        .barcode()
        .expect("No barcode set for merge-call-jsons task");
    let mut it = task.inputs();
    let _ = it.next(); // Throw array first element as this is the BCF file
    let json_files: Vec<_> = it
        .map(|x| {
            let asset = gem_bs.get_asset(*x).expect("Couldn't find JSON file asset");
            pipeline.add_remove_file(asset.path());
            (asset.id().to_owned(), asset.path().to_owned())
        })
        .collect();
    let com = QPipeCom::MergeCallJsons(MergeJsonFiles {
        barcode: bc.to_owned(),
        json_files,
    });
    pipeline.add_com(com);
    pipeline
}

pub fn merge_call_jsons(
    sig: Arc<AtomicUsize>,
    outputs: &[PathBuf],
    sfiles: &MergeJsonFiles,
) -> Result<(), String> {
    let mut combined_stats: Option<CallJson> = None;
    let mut it = sfiles.json_files.iter();
    let _ = it.next(); // Throw array first element as this is the BCF file
    for (_, path) in it {
        check_signal(Arc::clone(&sig))?;
        let rdr = compress::open_bufreader(path).map_err(|e| format!("{}", e))?;
        let jstats = CallJson::from_reader(rdr)?;
        combined_stats = if let Some(mut st) = combined_stats {
            st.merge(&jstats);
            Some(st)
        } else {
            Some(jstats)
        }
    }
    check_signal(sig)?;
    if let Some(st) = combined_stats {
        let output = outputs
            .first()
            .expect("No output file for merge JSON command");
        let wrt = compress::open_bufwriter(&output).map_err(|e| format!("{}", e))?;
        st.to_writer(wrt)?;
        Ok(())
    } else {
        Err("OK".to_string())
    }
}

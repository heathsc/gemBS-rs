use std::collections::HashMap;

use super::QPipe;
use crate::common::assets::GetAsset;
use crate::common::defs::{DataValue, Section, VarType};
use crate::config::GemBS;

fn make_gem_index(gem_bs: &GemBS, job: usize, bisulfite: bool) -> QPipe {
    let gembs_ref = gem_bs
        .get_asset("gembs_reference")
        .expect("Couldn't find gemBS reference asset");
    let index_name = if bisulfite { "index" } else { "nonbs_index" };
    let index = gem_bs
        .get_asset(index_name)
        .expect("Couldn't find gemBS index asset")
        .path();
    let index_base = index.to_string_lossy().clone();
    let index_base = index_base.trim_end_matches(".gem");
    let gem_indexer = gem_bs.get_exec_path("gem-indexer");
    let mut args = format!(
        "-i\x1e{}\x1e-o\x1e{}\x1e",
        gembs_ref.path().to_string_lossy(),
        index_base
    );
    if bisulfite {
        args.push_str("--bisulfite-index\x1e")
    }
    if let Some(x) = gem_bs.get_config_int(Section::Index, "sampling_rate") {
        args.push_str(format!("--text-sampling-rate\x1e{}\x1e", x).as_str())
    }
    if let Some(x) = gem_bs.get_config_int(Section::Index, "threads") {
        args.push_str(format!("--threads\x1e{}\x1e", x).as_str())
    }
    if let Some(x) = index.parent() {
        args.push_str(format!("--tmp-folder\x1e{}", x.to_string_lossy()).as_str())
    }
    let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
    if let Some(x) = gem_bs.get_tasks()[job].log() {
        pipeline.log = Some(
            gem_bs
                .get_asset(x)
                .expect("Couldn't get log file")
                .path()
                .to_owned(),
        )
    }
    if gem_bs.get_config_bool(Section::Index, "keep_logs") {
        pipeline.set_remove_log(false)
    }
    for out in gem_bs.get_tasks()[job].outputs() {
        pipeline.add_outputs(
            gem_bs
                .get_asset(*out)
                .expect("Couldn't get index output asset")
                .path(),
        );
    }
    pipeline.add_stage(&gem_indexer, &args);
    pipeline
}

fn make_dbsnp_index(
    gem_bs: &GemBS,
    _options: &HashMap<&'static str, (DataValue, bool)>,
    job: usize,
) -> QPipe {
    let index = gem_bs
        .get_asset("dbsnp_index")
        .expect("Couldn't find dbSNP index asset")
        .path();
    let dbsnp_index = gem_bs.get_exec_path("dbsnp_index");
    let task = &gem_bs.get_tasks()[job];
    let mut args = format!("--loglevel\x1e{}\x1e", gem_bs.verbose());

    const OPT_LIST: &[(&str, &str, VarType)] = &[
        ("dbsnp_jobs", "jobs", VarType::Int),
        ("threads", "threads", VarType::Int),
        ("dbsnp_selected", "selected", VarType::String),
        ("dbsnp_chrom_alias", "chrom-alias", VarType::String),
        ("dbsnp_type", "type", VarType::DbSnpFileType),
    ];

    super::add_command_opts(gem_bs, &mut args, Section::DbSnp, OPT_LIST);
    args.push_str(format!("--output\x1e{}", index.display()).as_str());
    for asset in task
        .inputs()
        .map(|ix| gem_bs.get_asset(*ix).expect("Missing asset"))
        .filter(|asset| asset.id().starts_with("dbsnp_file_"))
    {
        args.push_str(format!("\x1e{}", asset.path().display()).as_str())
    }
    let mut pipeline = QPipe::new(gem_bs.get_signal_clone());
    if let Some(x) = task.log() {
        pipeline.log = Some(
            gem_bs
                .get_asset(x)
                .expect("Couldn't get log file")
                .path()
                .to_owned(),
        )
    }
    let keep_logs = if gem_bs.keep_logs() {
        true
    } else if let Some(DataValue::Bool(x)) = gem_bs
        .get_config(Section::DbSnp, "keep_logs")
        .or_else(|| gem_bs.get_config(Section::Index, "keep_logs"))
    {
        *x
    } else {
        false
    };
    if keep_logs {
        pipeline.set_remove_log(false)
    }
    for out in task.outputs() {
        pipeline.add_outputs(
            gem_bs
                .get_asset(*out)
                .expect("Couldn't get dbsnp_index output asset")
                .path(),
        );
    }
    pipeline.add_stage(&dbsnp_index, &args);
    pipeline
}

pub fn make_index_pipeline(
    gem_bs: &GemBS,
    options: &HashMap<&'static str, (DataValue, bool)>,
    job: usize,
) -> QPipe {
    // First check what type of index we are meant to be building
    match gem_bs.get_tasks()[job].id() {
        "index" => make_gem_index(gem_bs, job, true),
        "nonbs_index" => make_gem_index(gem_bs, job, false),
        "dbsnp_index" => make_dbsnp_index(gem_bs, options, job),
        _ => panic!("Unknown index task"),
    }
}

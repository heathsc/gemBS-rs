use crate::cli::utils::handle_options;
use crate::common::assets::GetAsset;
use crate::common::defs::{Command, DataValue, Metadata, Section};
use crate::common::{dry_run, utils};
use crate::config::GemBS;
use crate::scheduler;
use clap::ArgMatches;
use std::collections::{HashMap, HashSet};

fn get_required_asset_list(
    gem_bs: &GemBS,
    options: &HashMap<&'static str, DataValue>,
) -> Result<HashSet<usize>, String> {
    let make_cram = gem_bs.get_config_bool(Section::Mapping, "make_cram");
    let suffix = if make_cram { "cram" } else { "bam" };
    let mut asset_ids = HashSet::new();
    if let Some(DataValue::StringVec(dvec)) = options.get("_dataset") {
        for dataset in dvec.iter() {
            if let Some(asset) = gem_bs
                .get_asset(format!("{}.bam", dataset).as_str())
                .or_else(|| {
                    if let Some(DataValue::String(bc)) = gem_bs
                        .get_sample_data_ref()
                        .get(dataset)
                        .and_then(|rf| rf.get(&Metadata::SampleBarcode))
                    {
                        gem_bs.get_asset(format!("{}.{}", bc, suffix).as_str())
                    } else {
                        None
                    }
                })
            {
                asset_ids.insert(asset.idx());
            } else {
                return Err(format!("Unknown dataset {}", dataset));
            }
        }
    } else if let Some(DataValue::StringVec(bvec)) = options.get("_barcode") {
        for barcode in bvec.iter() {
            let mut id = format!("{}.{}", barcode, suffix);
            if let Some(asset) = gem_bs.get_asset(id.as_str()) {
                asset_ids.insert(asset.idx());
            } else {
                return Err(format!("Unknown barcode {}", barcode));
            }
            id.push_str(".md5");
            asset_ids.insert(
                gem_bs
                    .get_asset(id.as_str())
                    .expect("Couldn't get md5 file for BAM")
                    .idx(),
            );
        }
    } else if let Some(DataValue::StringVec(svec)) = options.get("_sample") {
        for sample in svec.iter() {
            let mut asset = None;
            for hr in gem_bs.get_sample_data_ref().values() {
                if let Some(DataValue::String(x)) = hr.get(&Metadata::SampleName) {
                    if x == sample {
                        if let Some(DataValue::String(bc)) = hr.get(&Metadata::SampleBarcode) {
                            asset = gem_bs.get_asset(format!("{}.{}", bc, suffix).as_str());
                            break;
                        }
                    }
                }
            }
            if let Some(a) = asset {
                asset_ids.insert(a.idx());
            } else {
                return Err(format!("Unknown sample {}", sample));
            }
        }
    } else {
        let mut samples = HashSet::new();
        for hr in gem_bs.get_sample_data_ref().values() {
            if let Some(DataValue::String(bc)) = hr.get(&Metadata::SampleBarcode) {
                samples.insert(bc);
            }
        }
        for bc in samples.iter() {
            if let Some(asset) = gem_bs.get_asset(format!("{}.{}", bc, suffix).as_str()) {
                asset_ids.insert(asset.idx());
            } else {
                return Err(format!("Missing asset for barcode {}", bc));
            }
        }
    }
    Ok(asset_ids)
}

fn gen_map_command(
    gem_bs: &mut GemBS,
    options: &HashMap<&'static str, DataValue>,
) -> Result<(), String> {
    let task_path = gem_bs.get_task_file_path();
    let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?;
    gem_bs.setup_assets_and_tasks(&flock)?;
    let mut assets = get_required_asset_list(gem_bs, &options)?;
    let mut coms = HashSet::new();
    if !options.contains_key("_no_md5") {
        super::md5sum::get_assets_md5_map(gem_bs, &options, &mut assets, &mut coms)?;
    }
    if gem_bs.all() {
        [Command::Index, Command::Map].iter().for_each(|x| {
            coms.insert(*x);
        })
    } else if !(options.contains_key("_merge") || options.contains_key("_md5")) {
        coms.insert(Command::Map);
    }
    if !options.contains_key("_no_merge") {
        coms.insert(Command::MergeBams);
    }
    let asset_ids: Vec<_> = assets.iter().copied().collect();
    let com_set: Vec<_> = coms.iter().copied().collect();
    let task_list = gem_bs.get_required_tasks_from_asset_list(&asset_ids, &com_set);
    if gem_bs.execute_flag() {
        scheduler::schedule_jobs(gem_bs, &options, &task_list, &asset_ids, &com_set, flock)
    } else {
        dry_run::handle_nonexec(gem_bs, &options, &task_list)
    }
}

pub fn map_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
    gem_bs.setup_fs(false)?;
    gem_bs.read_config()?;

    let mut options = handle_options(m, gem_bs, Section::Mapping);
    if options.contains_key("_no_merge") {
        options.insert("_no_md5", DataValue::Bool(true));
    }
    if options.contains_key("_dataset") {
        options.insert("_no_md5", DataValue::Bool(true));
        options.insert("_no_merge", DataValue::Bool(true));
    }
    if options.contains_key("_md5") {
        options.insert("_no_merge", DataValue::Bool(true));
    }
    gen_map_command(gem_bs, &options)
}

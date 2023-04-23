use crate::cli::utils::handle_options;
use crate::common::assets::GetAsset;
use crate::common::defs::{Command, DataValue, Section};
use crate::common::{dry_run, utils};
use crate::config::GemBS;
use crate::scheduler;
use clap::ArgMatches;
use std::collections::HashMap;

fn get_required_asset_list(
    gem_bs: &GemBS,
    options: &HashMap<&'static str, (DataValue, bool)>,
) -> Result<Vec<usize>, String> {
    let barcodes = super::get_barcode_list(gem_bs, options)?;
    let mut asset_list = Vec::new();
    for bc in barcodes.iter() {
        if gem_bs.get_config_bool(Section::Extract, "make_cpg") {
            asset_list.push(
                gem_bs
                    .get_asset(format!("{}_cpg.txt.gz", bc).as_str())
                    .unwrap()
                    .idx(),
            )
        }
        if gem_bs.get_config_bool(Section::Extract, "make_non_cpg") {
            asset_list.push(
                gem_bs
                    .get_asset(format!("{}_non_cpg.txt.gz", bc).as_str())
                    .unwrap()
                    .idx(),
            )
        }
        if gem_bs.get_config_bool(Section::Extract, "make_bedmthyl") {
            asset_list.push(
                gem_bs
                    .get_asset(format!("{}_cpg.bed.gz", bc).as_str())
                    .unwrap()
                    .idx(),
            )
        }
        if gem_bs.get_config_bool(Section::Extract, "make_snps") {
            asset_list.push(
                gem_bs
                    .get_asset(format!("{}_snps.txt.gz", bc).as_str())
                    .unwrap()
                    .idx(),
            )
        }
    }
    Ok(asset_list)
}

pub fn extract_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
    gem_bs.setup_fs(false)?;
    gem_bs.read_config()?;

    let options = handle_options(m, gem_bs, Section::Extract);
    let task_path = gem_bs.get_task_file_path();
    let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?;
    gem_bs.setup_assets_and_tasks(&flock)?;
    let asset_ids = get_required_asset_list(gem_bs, &options)?;
    let task_list = if gem_bs.all() {
        gem_bs.get_required_tasks_from_asset_list(
            &asset_ids,
            &[
                Command::Index,
                Command::Map,
                Command::MergeBams,
                Command::Call,
                Command::MergeBcfs,
                Command::MD5SumMap,
                Command::MD5SumCall,
                Command::IndexBcf,
                Command::Extract,
            ],
        )
    } else {
        gem_bs.get_required_tasks_from_asset_list(&asset_ids, &[Command::Extract])
    };
    if gem_bs.execute_flag() {
        scheduler::schedule_jobs(
            gem_bs,
            &options,
            &task_list,
            &asset_ids,
            &[Command::Extract],
            flock,
        )
    } else {
        dry_run::handle_nonexec(gem_bs, &options, &task_list)
    }
}

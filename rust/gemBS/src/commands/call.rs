use crate::cli::utils::handle_options;
use crate::common::assets::GetAsset;
use crate::common::defs::{Command, DataValue, Section};
use crate::common::dry_run;
use crate::common::utils;
use crate::config::{contig, GemBS};
use crate::scheduler;
use clap::ArgMatches;
use std::collections::{HashMap, HashSet};

fn get_required_asset_list(
    gem_bs: &GemBS,
    options: &HashMap<&'static str, (DataValue, bool)>,
) -> Result<HashSet<usize>, String> {
    let barcodes = super::get_barcode_list(gem_bs, options)?;
    let pools = contig::get_contig_pools(gem_bs);

    // First we add the merged BCFs if required
    let mut asset_ids = HashSet::new();
    if !options.contains_key("no_merge") || pools.len() == 1 {
        if !options.contains_key("index") {
            for barcode in barcodes.iter() {
                if let Some(asset) = gem_bs.get_asset(format!("{}.bcf", barcode).as_str()) {
                    asset_ids.insert(asset.idx());
                } else {
                    return Err(format!("Unknown barcode {}", barcode));
                }
                asset_ids.insert(
                    gem_bs
                        .get_asset(format!("{}_call.json", barcode).as_str())
                        .expect("Couldn't get call JSON asset")
                        .idx(),
                );
            }
        }
        if !options.contains_key("no_index") {
            for barcode in barcodes.iter() {
                asset_ids.insert(
                    gem_bs
                        .get_asset(format!("{}.bcf.csi", barcode).as_str())
                        .expect("Couldn't get bcf index asset")
                        .idx(),
                );
            }
        }
    }
    // Now the individual contig pools
    if !options.contains_key("merge") && pools.len() > 1 {
        let add_bcf_asset = |b: &str, p: &str, rf: &mut HashSet<usize>| {
            if let Some(asset) = gem_bs.get_asset(format!("{}_{}.bcf", b, p).as_str()) {
                rf.insert(asset.idx());
            } else {
                return Err(format!("Unknown pool {}", p));
            }
            if let Some(asset) = gem_bs.get_asset(format!("{}_{}_call.json", b, p).as_str()) {
                rf.insert(asset.idx());
                Ok(())
            } else {
                Err("Couldn't get pool JSON asset".to_string())
            }
        };
        let add_pool_asset = |p: &str, rf: &mut HashSet<usize>| -> Result<(), String> {
            for barcode in barcodes.iter() {
                add_bcf_asset(barcode, p, rf)?
            }
            Ok(())
        };
        if let Some((DataValue::StringVec(vpool), _)) = options.get("pool") {
            for pool in vpool.iter() {
                add_pool_asset(pool, &mut asset_ids)?
            }
        } else {
            for pool in pools.iter() {
                add_pool_asset(pool, &mut asset_ids)?;
            }
        }
    }
    Ok(asset_ids)
}

fn gen_call_command(
    gem_bs: &mut GemBS,
    options: &HashMap<&'static str, (DataValue, bool)>,
) -> Result<(), String> {
    let task_path = gem_bs.get_task_file_path();
    let flock = utils::wait_for_lock(gem_bs.get_signal_clone(), &task_path)?;
    gem_bs.setup_assets_and_tasks(&flock)?;
    let mut assets = get_required_asset_list(gem_bs, &options)?;
    let mut coms = HashSet::new();
    if !options.contains_key("no_md5") {
        super::md5sum::get_assets_md5_call(gem_bs, &options, &mut assets, &mut coms)?;
    }
    if gem_bs.all() {
        [
            Command::Index,
            Command::Map,
            Command::MergeBams,
            Command::MD5SumMap,
            Command::Call,
        ]
        .iter()
        .for_each(|x| {
            coms.insert(*x);
        })
    } else if !(options.contains_key("merge")
        || options.contains_key("index")
        || options.contains_key("md5"))
    {
        coms.insert(Command::Call);
    }
    if !(options.contains_key("no_merge")
        || options.contains_key("index")
        || options.contains_key("md5"))
    {
        coms.insert(Command::MergeBcfs);
        coms.insert(Command::MergeCallJsons);
    }
    if !options.contains_key("no_index") {
        coms.insert(Command::IndexBcf);
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

pub fn call_command(m: &ArgMatches, gem_bs: &mut GemBS) -> Result<(), String> {
    gem_bs.setup_fs(false)?;
    gem_bs.read_config()?;

    let mut options = handle_options(m, gem_bs, Section::Calling);
    if options.contains_key("pool") {
        options.insert("no_merge", (DataValue::Bool(true), true));
    }
    if options.contains_key("no_merge") {
        options.insert("no_md5", (DataValue::Bool(true), true));
        options.insert("no_index", (DataValue::Bool(true), true));
    }
    if options.contains_key("md5") {
        options.insert("no_merge", (DataValue::Bool(true), true));
        options.insert("no_index", (DataValue::Bool(true), true));
    }
    gen_call_command(gem_bs, &options)
}

use std::collections::HashMap;
use std::io;

use super::cli_utils;
use crate::config::*;
use crate::{defs, htslib, process, reference};

use crate::dbsnp;
use clap::ArgMatches;

pub const OPTS: [(&str, ConfVar); 21] = [
    ("haploid", ConfVar::Bool(false)),
    ("keep_duplicates", ConfVar::Bool(false)),
    ("keep_supplementary", ConfVar::Bool(false)),
    ("ignore_duplicates", ConfVar::Bool(false)),
    ("keep_unmatched", ConfVar::Bool(false)),
    ("blank_trim", ConfVar::Bool(false)),
    ("benchmark_mode", ConfVar::Bool(false)),
    ("all_positions", ConfVar::Bool(false)),
    ("filter_contigs", ConfVar::Bool(false)),
    ("mapq_threshold", ConfVar::Int(20)),
    ("bq_threshold", ConfVar::Int(13)),
    ("max_template_length", ConfVar::Int(1000)),
    ("reference_bias", ConfVar::Float(2.0)),
    ("sample", ConfVar::String(None)),
    ("output", ConfVar::String(None)),
    ("reference", ConfVar::String(None)),
    ("contig_bed", ConfVar::String(None)),
    ("contig_exclude", ConfVar::String(None)),
    ("contig_include", ConfVar::String(None)),
    ("dbsnp", ConfVar::String(None)),
    ("report_file", ConfVar::String(None)),
];

fn distribute_threads(
    conf_hash: &mut HashMap<&'static str, ConfVar>,
    in_file: &mut htslib::SamFile,
    out_file: &mut htslib::VcfFile,
) -> io::Result<()> {
    let format = in_file.format();
    let input_compressed = format.is_compressed();
    let otype = if let Some(ConfVar::OType(x)) = conf_hash.get(&"output_type") {
        *x
    } else {
        panic!("Output_type config var not set");
    };
    let output_compressed = otype.is_compressed();
    let mut nn = 20;
    let mut k = if let Some(ConfVar::Int(x)) = conf_hash.get(&"threads") {
        *x
    } else {
        panic!("Integer config var threads not set");
    };
    if !input_compressed {
        nn -= 5
    }
    if !output_compressed {
        nn -= 5
    }
    let output_threads = if output_compressed {
        let x = k * 5 / nn;
        k -= x;
        x
    } else {
        0
    };
    let input_threads = if input_compressed {
        let x = k * 5 / nn;
        k -= x;
        x
    } else {
        0
    };
    let calc_threads = k;
    conf_hash.insert(&"calc_threads", ConfVar::Int(calc_threads));
    conf_hash.insert(&"input_threads", ConfVar::Int(input_threads));
    conf_hash.insert(&"output_threads", ConfVar::Int(output_threads));
    if input_threads > 0 {
        in_file.set_threads(input_threads)?
    }
    if output_threads > 0 {
        out_file.set_threads(output_threads)?
    }
    Ok(())
}

pub fn get_trim_values(m: &ArgMatches, name: &str) -> io::Result<(usize, usize)> {
    if let Some(v) = cli_utils::get_ivec(m, name, 0, 63)? {
        match v.len() {
            1 => Ok((v[0], v[0])),
            2 => Ok((v[0], v[1])),
            _ => Err(htslib::hts_err(
                "Unexpected number of values for trim option".to_string(),
            )),
        }
    } else {
        Ok((0, 0))
    }
}

pub fn handle_options(m: &ArgMatches) -> io::Result<(BsCallConfig, BsCallFiles)> {
    let mut conf_hash: HashMap<&'static str, ConfVar> = HashMap::new();
    // Handle simple options
    for (opt, val) in OPTS.iter() {
        let x = cli_utils::get_option(m, opt, val.clone())?;
        trace!("Inserting config option {} with value {:?}", opt, x);
        conf_hash.insert(opt, x);
    }

    // And now the odd options

    // Conversion rates
    let (under, over) = if let Some(v) = cli_utils::get_fvec(m, "conversion", 1.0e-8, 1.0 - 1.0e-8)?
    {
        (v[0], v[1])
    } else {
        (0.01, 0.05)
    };
    conf_hash.insert(&"under_conversion", ConfVar::Float(under));
    conf_hash.insert(&"over_conversion", ConfVar::Float(over));

    // Left and right trim
    let (x1, x2) = get_trim_values(m, "left_trim")?;
    conf_hash.insert(&"left_trim_read_1", ConfVar::Int(x1));
    conf_hash.insert(&"left_trim_read_2", ConfVar::Int(x2));
    let (x1, x2) = get_trim_values(m, "right_trim")?;
    conf_hash.insert(&"right_trim_read_1", ConfVar::Int(x1));
    conf_hash.insert(&"right_trim_read_2", ConfVar::Int(x2));

    // Output type - if not set we try to guess from output file name (if supplied), otherwise use VCF format
    let output = if let ConfVar::String(x) = conf_hash.get(&"output").unwrap() {
        x.as_deref().map(|x| x.to_owned())
    } else {
        panic!("String variable output not set")
    };
    let ocopy = output.clone();
    let otype = if let Some(ot) = m.get_one::<OType>("output_type").copied() {
        if !ot.eq_u32(htslib::FT_VCF)
            && output.is_none()
            && unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 }
        {
            warn!("Will not output binary and/or compressed data to terminal");
            OType::new(htslib::FT_VCF)
        } else {
            ot
        }
    } else if let Some(x) = output {
        if x.ends_with(".bcf") || x.ends_with(".bcf.gz") {
            OType::new(htslib::FT_BCF_GZ)
        } else if x.ends_with(".vcf.gz") {
            OType::new(htslib::FT_VCF_GZ)
        } else {
            OType::new(htslib::FT_VCF)
        }
    } else {
        OType::new(htslib::FT_VCF)
    };
    conf_hash.insert(&"output_type", ConfVar::OType(otype));

    // Input file
    let mut in_file = process::open_sam_input(m.get_one::<String>("input").map(|s| s.as_ref()))?;

    // Output file
    let mut out_file = process::open_vcf_output(ocopy.as_deref(), otype)?;

    // Threads
    conf_hash.insert(
        &"threads",
        cli_utils::get_option(m, "threads", ConfVar::Int(num_cpus::get()))?,
    );
    distribute_threads(&mut conf_hash, &mut in_file, &mut out_file)?;

    let chash = ConfHash::new(conf_hash);
    // Reference
    let rf = chash.get_str(&"reference");
    let ref_idx = reference::handle_reference(rf.unwrap(), &mut in_file)?;

    // dbSNP index
    let dbsnp_index = if let Some(dbsnp_file) = chash.get_str(&"dbsnp") {
        Some(dbsnp::DBSnpIndex::new(dbsnp_file)?)
    } else {
        None
    };

    // Set up contigs and contig regions
    let (ctgs, ctg_regions) = defs::setup_contigs(&chash, &in_file, &ref_idx)?;
    in_file.set_region_itr(&ctg_regions);
    let bs_cfg = BsCallConfig::new(chash, ctgs, ctg_regions);
    let bs_files = BsCallFiles::new(in_file, out_file, ref_idx, dbsnp_index);

    Ok((bs_cfg, bs_files))
}

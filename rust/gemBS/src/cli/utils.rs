use clap::ArgMatches;
use std::collections::HashMap;

use crate::common::defs::{
    DataValue, DbSnpFileType, FileType, JobLen, MemSize, ReadEnd, Section, VarType,
};
use crate::common::latex_utils::PageSize;
use crate::config::GemBS;

pub fn get_option(m: &ArgMatches, opt: &str, tt: VarType) -> Option<DataValue> {
    match tt {
        VarType::Int => m.get_one::<isize>(opt).copied().map(DataValue::Int),
        VarType::IntVec => m
            .get_many::<isize>(opt)
            .map(|v| DataValue::IntVec(v.copied().collect())),
        VarType::Bool => {
            if m.contains_id(opt) {
                Some(DataValue::Bool(true))
            } else {
                None
            }
        }
        VarType::Float => m.get_one::<f64>(opt).copied().map(DataValue::Float),
        VarType::String => m.get_one::<String>(opt).cloned().map(DataValue::String),
        VarType::FileType => m.get_one::<FileType>(opt).copied().map(DataValue::FileType),
        VarType::ReadEnd => m.get_one::<ReadEnd>(opt).copied().map(DataValue::ReadEnd),
        VarType::JobLen => m.get_one::<JobLen>(opt).copied().map(DataValue::JobLen),
        VarType::PageSize => m.get_one::<PageSize>(opt).copied().map(DataValue::PageSize),
        VarType::MemSize => m.get_one::<MemSize>(opt).copied().map(DataValue::MemSize),
        VarType::DbSnpFileType => m
            .get_one::<DbSnpFileType>(opt)
            .copied()
            .map(DataValue::DbSnpFileType),
        VarType::FloatVec => m
            .get_many::<f64>(opt)
            .map(|v| DataValue::FloatVec(v.copied().collect())),
        VarType::StringVec => m
            .get_many::<String>(opt)
            .map(|v| DataValue::StringVec(v.cloned().collect())),
    }
}

pub fn handle_options(
    m: &ArgMatches,
    gem_bs: &mut GemBS,
    section: Section,
) -> HashMap<&'static str, DataValue> {
    let mut options = HashMap::new();
    for (opt, val) in OPT_ASSOC.iter() {
        match val {
            OptionType::Global(s, vt) => {
                if let Some(x) = get_option(m, opt, *vt) {
                    options.insert(*opt, x.clone());
                    gem_bs.set_config(section, s, x);
                }
            }
            OptionType::Local(vt) => {
                if let Some(x) = get_option(m, opt, *vt) {
                    debug!("Setting local option {} to {:?}", opt, x);
                    options.insert(*opt, x);
                }
            }
            OptionType::Special(s, vt) => {
                if let Some(x) = get_option(m, opt, *vt) {
                    debug!("Setting special option {} to {:?}", opt, x);
                    options.insert(s, x);
                }
            }
        }
    }
    options
}

#[derive(Debug, Copy, Clone)]
pub enum OptionType {
    Global(&'static str, VarType),
    Local(VarType),
    Special(&'static str, VarType),
}

pub static OPT_ASSOC: &[(&str, OptionType)] = &[
    ("threads", OptionType::Global("threads", VarType::Int)),
    (
        "map_threads",
        OptionType::Global("map_threads", VarType::Int),
    ),
    (
        "merge_threads",
        OptionType::Global("merge_threads", VarType::Int),
    ),
    (
        "sort_threads",
        OptionType::Global("sort_threads", VarType::Int),
    ),
    (
        "call_threads",
        OptionType::Global("call_threads", VarType::Int),
    ),
    ("cores", OptionType::Global("cores", VarType::Int)),
    ("time", OptionType::Global("time", VarType::JobLen)),
    ("memory", OptionType::Global("memory", VarType::MemSize)),
    (
        "sort_memory",
        OptionType::Global("sort_memory", VarType::MemSize),
    ),
    ("tmp_dir", OptionType::Global("tmp_dir", VarType::String)),
    (
        "underconv_seq",
        OptionType::Global("underconversion_sequence", VarType::String),
    ),
    (
        "overconv_seq",
        OptionType::Global("overconversion_sequence", VarType::String),
    ),
    (
        "reverse",
        OptionType::Global("reverse_conversion", VarType::Bool),
    ),
    (
        "non_stranded",
        OptionType::Global("non_stranded", VarType::Bool),
    ),
    (
        "benchmark_mode",
        OptionType::Global("benchmark_mode", VarType::Bool),
    ),
    ("jobs", OptionType::Global("jobs", VarType::Int)),
    ("non_bs", OptionType::Local(VarType::Bool)),
    ("bs", OptionType::Local(VarType::Bool)),
    ("merge", OptionType::Special("_merge", VarType::Bool)),
    ("no_merge", OptionType::Special("_no_merge", VarType::Bool)),
    ("md5", OptionType::Special("_md5", VarType::Bool)),
    ("no_md5", OptionType::Special("_no_md5", VarType::Bool)),
    ("index", OptionType::Special("_index", VarType::Bool)),
    ("no_index", OptionType::Special("_no_index", VarType::Bool)),
    ("merge", OptionType::Local(VarType::Bool)),
    ("remove", OptionType::Local(VarType::Bool)),
    ("paired", OptionType::Local(VarType::Bool)),
    ("file_type", OptionType::Local(VarType::FileType)),
    ("sample", OptionType::Special("_sample", VarType::StringVec)),
    (
        "barcode",
        OptionType::Special("_barcode", VarType::StringVec),
    ),
    (
        "dataset",
        OptionType::Special("_dataset", VarType::StringVec),
    ),
    (
        "dbsnp_index",
        OptionType::Global("dbsnp_index", VarType::StringVec),
    ),
    (
        "list_pools",
        OptionType::Special("_list_pools", VarType::Int),
    ),
    ("pool", OptionType::Special("_pool", VarType::StringVec)),
    ("haploid", OptionType::Global("haploid", VarType::Bool)),
    (
        "keep_duplicates",
        OptionType::Global("keep_duplicates", VarType::Bool),
    ),
    ("keep_logs", OptionType::Global("keep_logs", VarType::Bool)),
    (
        "ignore_duplicate_flag",
        OptionType::Global("ignore_duplicate_flag", VarType::Bool),
    ),
    (
        "keep_unmatched",
        OptionType::Global("keep_improper_pairs", VarType::Bool),
    ),
    (
        "mapq_threshold",
        OptionType::Global("mapq_threshold", VarType::Int),
    ),
    (
        "qual_threshold",
        OptionType::Global("qual_threshold", VarType::Int),
    ),
    (
        "phred_threshold",
        OptionType::Global("phred_threshold", VarType::Int),
    ),
    (
        "left_trim",
        OptionType::Global("left_trim", VarType::IntVec),
    ),
    (
        "right_trim",
        OptionType::Global("right_trim", VarType::IntVec),
    ),
    (
        "max_template_size",
        OptionType::Global("max_template_size", VarType::IntVec),
    ),
    (
        "conversion",
        OptionType::Global("conversion", VarType::FloatVec),
    ),
    (
        "auto_conversion",
        OptionType::Global("auto_conversion", VarType::Bool),
    ),
    (
        "ref_bias",
        OptionType::Global("reference_bias", VarType::Float),
    ),
    (
        "strand_specific",
        OptionType::Global("strand_specific", VarType::Bool),
    ),
    (
        "bigwig_strand_specific",
        OptionType::Global("bigwig_strand_specific", VarType::Bool),
    ),
    ("min_inform", OptionType::Global("min_inform", VarType::Int)),
    ("min_nc", OptionType::Global("min_nc", VarType::Int)),
    ("allow_het", OptionType::Global("allow_het", VarType::Bool)),
    (
        "ref_bias",
        OptionType::Global("reference_bias", VarType::Float),
    ),
    ("cpg", OptionType::Global("make_cpg", VarType::Bool)),
    ("non_cpg", OptionType::Global("make_non_cpg", VarType::Bool)),
    (
        "bed_methyl",
        OptionType::Global("make_bedmethyl", VarType::Bool),
    ),
    ("snps", OptionType::Global("make_snps", VarType::Bool)),
    ("snp_list", OptionType::Global("snp_list", VarType::String)),
    ("snp_db", OptionType::Global("dbsnp_index", VarType::String)),
    (
        "sampling",
        OptionType::Global("sampling_rate", VarType::Int),
    ),
    (
        "min_contig_size",
        OptionType::Global("min_contig_size", VarType::Int),
    ),
    ("make_bs_index", OptionType::Local(VarType::Bool)),
    ("make_nonbs_index", OptionType::Local(VarType::Bool)),
    ("make_dbsnp_index", OptionType::Local(VarType::Bool)),
    (
        "dbsnp_files",
        OptionType::Global("dbsnp_files", VarType::StringVec),
    ),
    (
        "dbsnp_chrom_alias",
        OptionType::Global("dbsnp_chrom_alias", VarType::StringVec),
    ),
    (
        "dbsnp_selected",
        OptionType::Global("dbsnp_selected", VarType::StringVec),
    ),
    (
        "dbsnp_type",
        OptionType::Global("dbsnp_type", VarType::DbSnpFileType),
    ),
    ("dbsnp_jobs", OptionType::Global("dbsnp_jobs", VarType::Int)),
    ("project", OptionType::Global("project", VarType::String)),
    (
        "report_dir",
        OptionType::Global("report_dir", VarType::String),
    ),
    (
        "paper_size",
        OptionType::Global("paper_size", VarType::PageSize),
    ),
    ("mapping", OptionType::Special("_mapping", VarType::Bool)),
    ("calling", OptionType::Special("_calling", VarType::Bool)),
    ("pdf", OptionType::Special("_pdf", VarType::Bool)),
    ("confirm", OptionType::Special("_confirm", VarType::Bool)),
    ("force", OptionType::Special("_force", VarType::Bool)),
];

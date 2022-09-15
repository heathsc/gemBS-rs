use clap::ArgMatches;
use std::collections::HashMap;
use std::str::FromStr;

use lazy_static::lazy_static;

use crate::common::defs::{
    DataValue, DbSnpFileType, FileType, JobLen, MemSize, ReadEnd, Section, VarType,
};
use crate::common::latex_utils::PageSize;
use crate::config::GemBS;

pub fn get_option(m: &ArgMatches, opt: &str, tt: VarType) -> Option<DataValue> {
    match tt {
        VarType::Int => m
            .value_of(opt)
            .and_then(|x| <isize>::from_str(x).ok().map(DataValue::Int)),
        VarType::IntVec => m.values_of(opt).map(|v| {
            let vec: Vec<_> = v.map(|x| <isize>::from_str(x).ok().unwrap()).collect();
            DataValue::IntVec(vec)
        }),
        VarType::Bool => {
            if m.is_present(opt) {
                Some(DataValue::Bool(true))
            } else {
                None
            }
        }
        VarType::Float => m
            .value_of(opt)
            .and_then(|x| <f64>::from_str(x).ok().map(DataValue::Float)),
        VarType::String => m.value_of(opt).map(|x| DataValue::String(x.to_owned())),
        VarType::FileType => m
            .value_of(opt)
            .and_then(|x| <FileType>::from_str(x).ok().map(DataValue::FileType)),
        VarType::ReadEnd => m
            .value_of(opt)
            .and_then(|x| <ReadEnd>::from_str(x).ok().map(DataValue::ReadEnd)),
        VarType::JobLen => m
            .value_of(opt)
            .and_then(|x| <JobLen>::from_str(x).ok().map(DataValue::JobLen)),
        VarType::PageSize => m
            .value_of(opt)
            .and_then(|x| <PageSize>::from_str(x).ok().map(DataValue::PageSize)),
        VarType::MemSize => m
            .value_of(opt)
            .and_then(|x| <MemSize>::from_str(x).ok().map(DataValue::MemSize)),
        VarType::DbSnpFileType => m.value_of(opt).and_then(|x| {
            <DbSnpFileType>::from_str(x)
                .ok()
                .map(DataValue::DbSnpFileType)
        }),
        VarType::FloatVec => m.values_of(opt).map(|v| {
            let vec: Vec<_> = v.map(|x| <f64>::from_str(x).ok().unwrap()).collect();
            DataValue::FloatVec(vec)
        }),
        VarType::StringVec => m.values_of(opt).map(|v| {
            let args: Vec<String> = v.map(|x| x.to_owned()).collect();
            DataValue::StringVec(args)
        }),
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

lazy_static! {
    pub static ref OPT_ASSOC: Vec<(&'static str, OptionType)> = {
        let mut m = Vec::new();
        m.push(("threads", OptionType::Global("threads", VarType::Int)));
        m.push((
            "map_threads",
            OptionType::Global("map_threads", VarType::Int),
        ));
        m.push((
            "merge_threads",
            OptionType::Global("merge_threads", VarType::Int),
        ));
        m.push((
            "sort_threads",
            OptionType::Global("sort_threads", VarType::Int),
        ));
        m.push((
            "call_threads",
            OptionType::Global("call_threads", VarType::Int),
        ));
        m.push(("cores", OptionType::Global("cores", VarType::Int)));
        m.push(("time", OptionType::Global("time", VarType::JobLen)));
        m.push(("memory", OptionType::Global("memory", VarType::MemSize)));
        m.push((
            "sort_memory",
            OptionType::Global("sort_memory", VarType::MemSize),
        ));
        m.push(("tmp_dir", OptionType::Global("tmp_dir", VarType::String)));
        m.push((
            "underconv_seq",
            OptionType::Global("underconversion_sequence", VarType::String),
        ));
        m.push((
            "overconv_seq",
            OptionType::Global("overconversion_sequence", VarType::String),
        ));
        m.push((
            "reverse",
            OptionType::Global("reverse_conversion", VarType::Bool),
        ));
        m.push((
            "non_stranded",
            OptionType::Global("non_stranded", VarType::Bool),
        ));
        m.push((
            "benchmark_mode",
            OptionType::Global("benchmark_mode", VarType::Bool),
        ));
        m.push(("jobs", OptionType::Global("jobs", VarType::Int)));
        m.push(("non_bs", OptionType::Local(VarType::Bool)));
        m.push(("bs", OptionType::Local(VarType::Bool)));
        m.push(("merge", OptionType::Special("_merge", VarType::Bool)));
        m.push(("no_merge", OptionType::Special("_no_merge", VarType::Bool)));
        m.push(("md5", OptionType::Special("_md5", VarType::Bool)));
        m.push(("no_md5", OptionType::Special("_no_md5", VarType::Bool)));
        m.push(("index", OptionType::Special("_index", VarType::Bool)));
        m.push(("no_index", OptionType::Special("_no_index", VarType::Bool)));
        m.push(("merge", OptionType::Local(VarType::Bool)));
        m.push(("remove", OptionType::Local(VarType::Bool)));
        m.push(("paired", OptionType::Local(VarType::Bool)));
        m.push(("file_type", OptionType::Local(VarType::FileType)));
        m.push(("sample", OptionType::Special("_sample", VarType::StringVec)));
        m.push((
            "barcode",
            OptionType::Special("_barcode", VarType::StringVec),
        ));
        m.push((
            "dataset",
            OptionType::Special("_dataset", VarType::StringVec),
        ));
        m.push((
            "dbsnp_index",
            OptionType::Global("dbsnp_index", VarType::StringVec),
        ));
        m.push((
            "list_pools",
            OptionType::Special("_list_pools", VarType::Int),
        ));
        m.push(("pool", OptionType::Special("_pool", VarType::StringVec)));
        m.push(("haploid", OptionType::Global("haploid", VarType::Bool)));
        m.push((
            "keep_duplicates",
            OptionType::Global("keep_duplicates", VarType::Bool),
        ));
        m.push(("keep_logs", OptionType::Global("keep_logs", VarType::Bool)));
        m.push((
            "ignore_duplicate_flag",
            OptionType::Global("ignore_duplicate_flag", VarType::Bool),
        ));
        m.push((
            "keep_unmatched",
            OptionType::Global("keep_improper_pairs", VarType::Bool),
        ));
        m.push((
            "mapq_threshold",
            OptionType::Global("mapq_threshold", VarType::Int),
        ));
        m.push((
            "qual_threshold",
            OptionType::Global("qual_threshold", VarType::Int),
        ));
        m.push((
            "phred_threshold",
            OptionType::Global("phred_threshold", VarType::Int),
        ));
        m.push((
            "left_trim",
            OptionType::Global("left_trim", VarType::IntVec),
        ));
        m.push((
            "right_trim",
            OptionType::Global("right_trim", VarType::IntVec),
        ));
        m.push((
            "max_template_size",
            OptionType::Global("max_template_size", VarType::IntVec),
        ));
        m.push((
            "conversion",
            OptionType::Global("conversion", VarType::FloatVec),
        ));
        m.push((
            "auto_conversion",
            OptionType::Global("auto_conversion", VarType::Bool),
        ));
        m.push((
            "ref_bias",
            OptionType::Global("reference_bias", VarType::Float),
        ));
        m.push((
            "strand_specific",
            OptionType::Global("strand_specific", VarType::Bool),
        ));
        m.push((
            "bigwig_strand_specific",
            OptionType::Global("bigwig_strand_specific", VarType::Bool),
        ));
        m.push(("min_inform", OptionType::Global("min_inform", VarType::Int)));
        m.push(("min_nc", OptionType::Global("min_nc", VarType::Int)));
        m.push(("allow_het", OptionType::Global("allow_het", VarType::Bool)));
        m.push((
            "ref_bias",
            OptionType::Global("reference_bias", VarType::Float),
        ));
        m.push(("cpg", OptionType::Global("make_cpg", VarType::Bool)));
        m.push(("non_cpg", OptionType::Global("make_non_cpg", VarType::Bool)));
        m.push((
            "bed_methyl",
            OptionType::Global("make_bedmethyl", VarType::Bool),
        ));
        m.push(("snps", OptionType::Global("make_snps", VarType::Bool)));
        m.push(("snp_list", OptionType::Global("snp_list", VarType::String)));
        m.push(("snp_db", OptionType::Global("dbsnp_index", VarType::String)));
        m.push((
            "sampling",
            OptionType::Global("sampling_rate", VarType::Int),
        ));
        m.push((
            "min_contig_size",
            OptionType::Global("min_contig_size", VarType::Int),
        ));
        m.push(("make_bs_index", OptionType::Local(VarType::Bool)));
        m.push(("make_nonbs_index", OptionType::Local(VarType::Bool)));
        m.push(("make_dbsnp_index", OptionType::Local(VarType::Bool)));
        m.push((
            "dbsnp_files",
            OptionType::Global("dbsnp_files", VarType::StringVec),
        ));
        m.push((
            "dbsnp_chrom_alias",
            OptionType::Global("dbsnp_chrom_alias", VarType::StringVec),
        ));
        m.push((
            "dbsnp_selected",
            OptionType::Global("dbsnp_selected", VarType::StringVec),
        ));
        m.push((
            "dbsnp_type",
            OptionType::Global("dbsnp_type", VarType::DbSnpFileType),
        ));
        m.push(("dbsnp_jobs", OptionType::Global("dbsnp_jobs", VarType::Int)));
        m.push(("project", OptionType::Global("project", VarType::String)));
        m.push((
            "report_dir",
            OptionType::Global("report_dir", VarType::String),
        ));
        m.push((
            "paper_size",
            OptionType::Global("paper_size", VarType::PageSize),
        ));
        m.push(("mapping", OptionType::Special("_mapping", VarType::Bool)));
        m.push(("calling", OptionType::Special("_calling", VarType::Bool)));
        m.push(("pdf", OptionType::Special("_pdf", VarType::Bool)));
        m.push(("confirm", OptionType::Special("_confirm", VarType::Bool)));
        m.push(("force", OptionType::Special("_force", VarType::Bool)));
        m
    };
}

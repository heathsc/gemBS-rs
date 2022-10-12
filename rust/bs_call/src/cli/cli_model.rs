use clap::{crate_version, value_parser, Arg, ArgAction, Command};

use utils::log_level::LogLevel;
use crate::config::OType;

pub(super) fn cli_model() -> Command {
    Command::new("bs_call")
        .version(crate_version!())
        .author("Simon Heath <simon.heath@gmail.com>")
        .about("bs_call is an application for calling methylation values and SNV from bisulfite treated sequence data")
        .next_help_heading("filtering")
        .arg(
            Arg::new("keep_duplicates")
                .action(ArgAction::SetTrue)
                .short('d')
                .long("keep-duplicates")
                .help("Don't merge duplicate reads"),
        )
        .arg(
            Arg::new("keep_supplementary")
                .action(ArgAction::SetTrue)
                .short('S')
                .long("keep-supplementary")
                .help("Don't discard supplementary (split) alignments"),
        )
        .arg(
            Arg::new("ignore_duplicates")
                .action(ArgAction::SetTrue)
                .short('e')
                .long("ignore-duplicates")
                .help("Ignore duplicate record flag in SAM record"),
        )
        .arg(
            Arg::new("keep_unmatched")
                .action(ArgAction::SetTrue)
                .short('k')
                .long("keep-unmatched")
                .help("Don't discard reads that don't form proper pairs"),
        )
        .arg(
            Arg::new("right_trim")
                .short('R')
                .long("right-trim")
                .value_name("INT")
                .value_parser(value_parser!(usize))
                .value_delimiter(',')
                .num_args(1..=2)
                .action(ArgAction::Append)
                .help("Bases to trim from right of read pair (give 2 values for read specific values)"),
        )
        .arg(
            Arg::new("left_trim")
                .short('L')
                .long("left-trim")
                .value_name("INT")
                .value_parser(value_parser!(usize))
                .value_delimiter(',')
                .action(ArgAction::Append)
                .num_args(1..=2)
                .help("Bases to trim from left of read pair (give 2 values for read specific values)"),
        )
        .arg(
            Arg::new("blank_trim")
                .action(ArgAction::SetTrue)
                .short('B')
                .long("blank-trim")
                .help("Don't use trimmed bases for genotype extimation"),
        )
        .arg(
            Arg::new("mapq_threshold")
                .short('q')
                .long("mapq-threshold")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .default_value("20")
                .help("Minimum threshold for read MAPQ values"),
        )
        .arg(
            Arg::new("bq_threshold")
                .short('Q')
                .long("bq-threshold")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .default_value("13")
                .help("Minimum threshold for base qualities"),
        )
        .arg(
            Arg::new("max_template_length")
                .short('l')
                .long("max-template-length")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .default_value("1000")
                .help("Maximum threshold for paired read template length"),
        )
        .arg(
            Arg::new("contig_bed")
                .short('C')
                .long("contig-bed")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("BED file with contigs to be processed"),
        )
        .arg(
            Arg::new("contig_include")
                .short('s')
                .long("contig-include")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("File with list of contigs to be included"),
        )
        .arg(
            Arg::new("contig_exclude")
                .short('x')
                .long("contig-exclude")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("File with list of contigs to be excluded"),
        )
        .arg(
            Arg::new("filter_contigs")
                .short('F')
                .long("filter_contigs")
                .action(ArgAction::SetTrue)
                .help("Only include in the outupt header contigs that are present in the contig_bed file")
                .requires("contig_bed")
        )
        .next_help_heading("calling")
        .arg(
            Arg::new("reference_bias")
                .short('R')
                .long("reference-bias")
                .value_parser(value_parser!(f64))
                .value_name("FLOAT")
                .default_value("2.0")
                .help("Set weight on reference homozygote"),
        )
        .arg(
            Arg::new("conversion")
                .short('c')
                .long("conversion")
                .value_name("FLOAT")
                .value_parser(value_parser!(f64))
                .num_args(1..=2)
                .value_delimiter(',')
                .action(ArgAction::Append)
                .default_values(&["0.01","0.05"])
                .help("Set under and over conversion rates"),
        )
        .arg(
            Arg::new("haploid")
                .short('1')
                .long("haploid")
                .action(ArgAction::SetTrue)
                .help("Force genotype calls to be haploid")
        )
        .next_help_heading("output")
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("Set output file name"),
        )
        .arg(
            Arg::new("output_type")
                .short('O')
                .long("output-type")
                .value_name("TYPE")
                .value_parser(value_parser!(OType))
                .ignore_case(true)
                .help("Set output type - b: compressed BCF (default); u: uncompressed BCF; v: uncompressed VCF, z: compressed VCF"),
        )
        .arg(
            Arg::new("report_file")
                .short('j')
                .long("report-file")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("File name for Output JSON with calling statistics"),
        )
        .arg(
            Arg::new("sample")
                .short('n')
                .long("sample")
                .value_parser(value_parser!(String))
                .value_name("STRING")
                .help("Set sample name"),
        )
        .arg(
            Arg::new("all_positions")
                .short('A')
                .action(ArgAction::SetTrue)
                .long("all-positions")
                .help("Output all covered positions")
        )
        .arg(
            Arg::new("benchmark_mode")
                .short('b')
                .action(ArgAction::SetTrue)
                .long("benchmark-mode")
                .help("Benchmark mode (omits date / time / version numbers from output)")
        )
        .next_help_heading("references")
        .arg(
            Arg::new("reference")
                .short('r')
                .long("reference")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .required(true)
                .help("Path to genomic reference (FASTA)"),
        )
        .arg(
            Arg::new("dbsnp")
                .short('D')
                .long("dbsnp")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("Path to dbsnp_index processed index file (to add known SNP names)"),
        )
        .next_help_heading("general")
        .arg(
            Arg::new("quiet")
                .short('q')
                .action(ArgAction::SetTrue)
                .long("quiet")
                .help("Silence all output"),
        )
        .arg(
            Arg::new("timestamp")
                .short('T')
                .long("timestamp")
                .value_parser(value_parser!(stderrlog::Timestamp))
                .value_name("GRANULARITY")
                .default_value("none")
                .help("Prepend log entries with a timestamp"),
        )
        .arg(
            Arg::new("loglevel")
                .short('v')
                .long("loglevel")
                .value_name("LOGLEVEL")
                .value_parser(value_parser!(LogLevel))
                .default_value("warn")
                .help("Set log level"),
        )
        .arg(
            Arg::new("threads")
                .short('t')
                .long("threads")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .help("Set number of threads"),
        )
        .arg(
            Arg::new("input")
                .value_name("PATH")
                .value_parser(value_parser!(String))
                .action(ArgAction::Append)
                .help("Input SAM/BAM/CRAM file"),
        )
}

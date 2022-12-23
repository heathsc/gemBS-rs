use clap::{crate_version, value_parser, Arg, ArgAction, ArgGroup, Command};

use utils::log_level::LogLevel;

pub(super) fn cli_model() -> Command {
    Command::new("mextr")
        .version(crate_version!())
        .author("Simon Heath <simon.heath@gmail.com>")
        .about("mextr is an application for extracting methylation information from VCF/BCF files")
        .next_help_heading("filtering (for gemBS bed output)")
        .arg(
            Arg::new("prop")
                .short('p')
                .long("prop")
                .value_parser(value_parser!(f64))
                .value_name("FLOAT")
                .default_value("0.0")
                .help("Minimum proportion of sites/CpGs that must pass filters"),
        )
        .arg(
            Arg::new("number")
                .short('N')
                .long("prop")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .default_value("1")
                .help("Minimum number of sites/CpGs that must pass filters"),
        )
        .arg(
            Arg::new("inform")
                .short('I')
                .long("inform")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .default_value("1")
                .help("Minimum number of informative reads for a site/CpG"),
        )
        .arg(
            Arg::new("threshold")
                .short('T')
                .long("threshold")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .default_value("20")
                .help("Minimum genotype PHRED score for a site/CpG"),
        )
        .arg(
            Arg::new("bq_threshold")
                .short('Q')
                .long("bq-threshold")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .default_value("20")
                .help("Minimum base quality score for genotype calling"),
        )
        .arg(
            Arg::new("select")
                .short('s')
                .long("select")
                .value_name("MODE")
                .value_parser(["hom", "het"])
                .default_value("hom")
                .ignore_case(true)
                .help("Minimum number of sites/CpGs that must pass filters"),
        )
        .arg(
            Arg::new("min_nc")
                .short('M')
                .long("min-nc")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .default_value("1")
                .help("Minimum number of non-converted bases for non CpG site"),
        )
        .arg(
            Arg::new("report_file")
                .short('S')
                .long("report-file")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("File name for Output JSON with calling statistics"),
        )
        .arg(
            Arg::new("region_list")
                .short('r')
                .long("regions")
                .value_parser(value_parser!(String))
                .action(ArgAction::Append)
                .value_delimiter(',')
                .value_name("REGION [,REGION...]")
                .help("Restrict to comma separated list of regions"),
        )
        .arg(
            Arg::new("regions_file")
                .short('R')
                .long("regions-file")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("Restrict to regions from file"),
        )
        .group(
            ArgGroup::new("region_def")
                .args(&["region_list", "regions_file"])
        )
        .next_help_heading("genotype recalling")
        .arg(
            Arg::new("reference_bias")
                .short('B')
                .long("reference-bias")
                .value_parser(value_parser!(f64))
                .value_name("FLOAT")
                .default_value("2.0")
                .help("Reference bias for genotype calling"),
        )
        .arg(
            Arg::new("conversion")
                .short('c')
                .long("conversion")
                .value_parser(value_parser!(f64))
                .action(ArgAction::Append)
                .num_args(1..=2)
                .value_name("FLOAT,FLOAT")
                .default_values(&["0.01", "0.05"])
                .value_delimiter(',')
                .help("Under and over conversion rates"),
        )
        .arg(
            Arg::new("common_gt")
                .action(ArgAction::SetTrue)
                .short('g')
                .long("common-gt")
                .help("Assume common genotypes across all samples"),
        )
        .arg(
            Arg::new("haploid")
                .action(ArgAction::SetTrue)
                .short('h')
                .long("haploid")
                .help("Force genotype calls to be haploid"),
        )
        .next_help_heading("output")
        .arg(
            Arg::new("cpgfile")
                .short('o')
                .long("cpgfile")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("File name for CpG file"),
        )
        .arg(
            Arg::new("noncpgfile")
                .short('n')
                .long("noncpgfile")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("File name for non CpG file"),
        )
        .arg(
            Arg::new("no_header")
                .short('H')
                .action(ArgAction::SetTrue)
                .long("no-header")
                .help("Do not output header line for gemBS bed files"),
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .value_name("MODE")
                .value_parser(["combined", "strand-specific"])
                .ignore_case(true)
                .default_value("combined")
                .help("Output mode for CpGs (gemBS bed)"),
        )
        .arg(
            Arg::new("bw_mode")
                .short('w')
                .long("bw-mode")
                .value_name("MODE")
                .value_parser(["combined", "strand-specific"])
                .ignore_case(true)
                .default_value("combined")
                .help("Output mode for bigWig files"),
        )
        .arg(
            Arg::new("bed_methyl")
                .short('b')
                .long("bedmethyl")
                .alias("bed-methyl")
                .value_parser(value_parser!(String))
                .value_name("STRING")
                .help("File base name for bedmethyl files (not compatible with multisample VCFs/BCFs)"),
        )
        .arg(
            Arg::new("bed_track_line")
                .short('t')
                .long("bed-track-line")
                .value_parser(value_parser!(String))
                .value_name("STRING")
                .help("Track line for bedMethyl files [default: info taken from input file]"),
        )
        .arg(
            Arg::new("compress")
                .short('z')
                .action(ArgAction::SetTrue)
                .long("compress")
                .help("Compress output file with bgzip"),
        )
        .arg(
            Arg::new("md5")
                .short('D')
                .action(ArgAction::SetTrue)
                .long("md5")
                .help("Generate md5 digest for output file"),
        )
        .arg(
            Arg::new("tabix")
                .short('x')
                .action(ArgAction::SetTrue)
                .long("tabix")
                .help("Generate tabix (tbx) index for output file"),
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
                .short('X')
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
                .ignore_case(true)
                .default_value("warn")
                .help("Set log level"),
        )
        .arg(
            Arg::new("threads")
                .short('@')
                .long("threads")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .help("Set number of threads"),
        )
        .arg(
            Arg::new("input")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .required(true)
                .help("Input VCF/BCF file"),
        )
        .arg(
            Arg::new("regions")
                .value_name("REGIONS")
                .value_parser(value_parser!(String))
                .action(ArgAction::Append)
                .value_delimiter(',')
                .help("Chromosome regions (comma separated)"),
        )
}

use clap::{crate_version, Arg, ArgAction, ArgGroup, Command};

use utils::log_level::LogLevel;

pub(super) fn cli_model() -> Command {
    Command::new("snpxtr")
        .version(crate_version!())
        .author("Simon Heath <simon.heath@gmail.com>")
        .about("snptr is an application for extracting SNPs from VCF/BCF files")
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .action(ArgAction::SetTrue)
                .help("Silence all output"),
        )
        .arg(
            Arg::new("timestamp")
                .short('T')
                .long("timestamp")
                .value_name("GRANULARITY")
                .value_parser(value_parser!(stderrlog::Timestamp))
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
                .short('@')
                .long("threads")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .help("Set number of threads"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("Set output file name [default: stdout]"),
        )
        .arg(
            Arg::new("selected")
                .short('s')
                .long("selected")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("File with list of selected SNPs [default: all SNPs are selected]"),
        )
        .arg(
            Arg::new("dbsnp")
                .short('D')
                .long("dbsnp")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("dbSNP index file (used to add external ids if not present in input file"),
        )
        .arg(
            Arg::new("region_list")
                .short('r')
                .long("regions")
                .value_delimiter(',')
                .value_parser(value_parser!(String))
                .action(ArgAction::Append)
                .value_name("REGION [,REGION...]")
                .help("Restrict to comma separated list of regions"),
        )
        .arg(
            Arg::new("regions_file")
                .short('R')
                .long("region-file")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("Restrict to regions from file"),
        )
        .group(
            ArgGroup::new("region_def")
                .args(&["region_list", "regions_file"])
                .required(true),
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
                .action(ArgAction::SetTrue)
                .short('m')
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
        .arg(
            Arg::new("input")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("Input VCF/BCF file"),
        )
        .arg(
            Arg::new("regions")
                .value_parser(value_parser!(String))
                .value_name("REGIONS")
                .value_delimiter(',')
                .action(ArgAction::Append)
                .help("Chromosome regions (comma separated)"),
        )
}

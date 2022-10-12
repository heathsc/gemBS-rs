use clap::{crate_version, Arg, Command, ArgAction, value_parser};
use utils::log_level::LogLevel;

pub(super) fn cli_model() -> Command {
    Command::new("dbsnp_index")
        .version(crate_version!())
        .author("Simon Heath <simon.heath@gmail.com>")
        .about("dbsnp_index is an application for generating an index of the dbSNP database to allow assigning public IDs to SNPs")
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
                .value_name("GRANULARITY")
                .value_parser(value_parser!(stderrlog::Timestamp))
                .default_value("none")
                .help("Prepend log entries with a timestamp"),
        )
        .arg(
            Arg::new("loglevel")
                .short('l')
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
            Arg::new("jobs")
                .short('j')
                .long("jobs")
                .value_parser(value_parser!(usize))
                .value_name("INT")
                .default_value("1")
                .help("Set number parallel file reading jobs"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .default_value("dbsnp.idx")
                .help("Output file"),
        )
        .arg(
            Arg::new("description")
                .short('d')
                .long("desc")
                .value_parser(value_parser!(String))
                .value_name("STRING")
                .help("Description of dataset"),
        )
        .arg(
            Arg::new("chrom_alias")
                .short('c')
                .long("chrom-alias")
                .value_parser(value_parser!(String))
                .value_name("PATH")
                .help("Chromosome name alias file"),
        )
        .arg(
            Arg::new("input_type")
                .short('X')
                .long("type")
                .value_name("TYPE")
                .value_parser(["AUTO", "BED", "JSON", "VCF"])
                .ignore_case(true)
                .default_value("AUTO")
                .help("Input file type"),
        )
        .arg(
            Arg::new("maf_limit")
                .short('m')
                .long("maf-limit")
                .value_parser(value_parser!(f64))
                .value_name("FLOAT")
                .help("Flags SNP with maf >= VALUE so that these position will always be reported in the output VCF from bs_call"),
        )
        .arg(
            Arg::new("selected")
                .short('s')
                .long("selected")
                .value_name("PATH")
                .help("File with list of SNPs (one per line) to be flagged as for --maf above"),
        )
        .arg(
            Arg::new("input")
                .action(ArgAction::Append)
                .value_parser(value_parser!(String))
                .value_name("FILE")
                .help("Input BED/VCF/JSON files [default: <stdin>]"),
        )
}

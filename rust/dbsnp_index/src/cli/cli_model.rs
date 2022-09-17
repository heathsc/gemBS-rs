use clap::{crate_version, App, Arg, Command};

pub(super) fn cli_model() -> App<'static> {
    Command::new("dbsnp_index")
        .version(crate_version!())
        .author("Simon Heath <simon.heath@gmail.com>")
        .about("dbsnp_index is an application for generating an index of the dbSNP database to allow assigning public IDs to SNPs")
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Silence all output"),
        )
        .arg(
            Arg::new("timestamp")
                .short('T')
                .long("timestamp")
                .takes_value(true)
                .value_name("GRANULARITY")
                .possible_values(&["none", "sec", "ms", "us", "ns"])
                .default_value("none")
                .help("Prepend log entries with a timestamp"),
        )
        .arg(
            Arg::new("loglevel")
                .short('v')
                .long("loglevel")
                .takes_value(true)
                .value_name("LOGLEVEL")
                .possible_values(&["none", "error", "warn", "info", "debug", "trace"])
                .ignore_case(true)
                .default_value("warn")
                .help("Set log level"),
        )
        .arg(
            Arg::new("threads")
                .short('t')
                .long("threads")
                .takes_value(true)
                .value_name("INT")
                .help("Set number of threads"),
        )
        .arg(
            Arg::new("jobs")
                .short('j')
                .long("jobs")
                .takes_value(true)
                .value_name("INT")
                .default_value("1")
                .help("Set number parallel file reading jobs"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .takes_value(true)
                .value_name("PATH")
                .default_value("dbsnp.idx")
                .help("Output file"),
        )
        .arg(
            Arg::new("description")
                .short('d')
                .long("desc")
                .takes_value(true)
                .value_name("STRING")
                .help("Description of dataset"),
        )
        .arg(
            Arg::new("chrom_alias")
                .short('c')
                .long("chrom-alias")
                .takes_value(true)
                .value_name("PATH")
                .help("Chromosome name alias file"),
        )
        .arg(
            Arg::new("input_type")
                .short('X')
                .long("type")
                .takes_value(true)
                .value_name("TYPE")
                .possible_values(&["AUTO", "BED", "JSON", "VCF"])
                .ignore_case(true)
                .default_value("AUTO")
                .help("Input file type"),
        )
        .arg(
            Arg::new("maf_limit")
                .short('m')
                .long("maf-limit")
                .takes_value(true)
                .value_name("FLOAT")
                .help("Flags SNP with maf >= VALUE so that these position will always be reported in the output VCF from bs_call"),
        )
        .arg(
            Arg::new("selected")
                .short('s')
                .long("selected")
                .takes_value(true)
                .value_name("PATH")
                .help("File with list of SNPs (one per line) to be flagged as for --maf above"),
        )
        .arg(
            Arg::new("input")
                .multiple_values(true)
                .takes_value(true)
                .value_name("FILE")
                .help("Input BED/VCF/JSON files [default: <stdin>]"),
        )
}

use clap::{crate_version, App, Arg, ArgGroup, Command};

pub(super) fn cli_model() -> App<'static> {
    Command::new("snpxtr")
        .version(crate_version!())
        .author("Simon Heath <simon.heath@gmail.com>")
        .about("snptr is an application for extracting SNPs from VCF/BCF files")
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
                .short('@')
                .long("threads")
                .takes_value(true)
                .value_name("INT")
                .help("Set number of threads"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .takes_value(true)
                .value_name("PATH")
                .help("Set output file name [default: stdout]"),
        )
        .arg(
            Arg::new("selected")
                .short('s')
                .long("selected")
                .takes_value(true)
                .value_name("PATH")
                .help("File with list of selected SNPs [default: all SNPs are selected]"),
        )
        .arg(
            Arg::new("dbsnp")
                .short('D')
                .long("dbsnp")
                .takes_value(true)
                .value_name("PATH")
                .help("dbSNP index file (used to add external ids if not present in input file"),
        )
        .arg(
            Arg::new("region_list")
                .short('r')
                .long("regions")
                .takes_value(true)
                .multiple_values(true)
                .require_value_delimiter(true)
                .value_name("REGION [,REGION...]")
                .help("Restrict to comma separated list of regions"),
        )
        .arg(
            Arg::new("regions_file")
                .short('R')
                .long("region-file")
                .takes_value(true)
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
                .long("compress")
                .help("Compress output file with bgzip"),
        )
        .arg(
            Arg::new("md5")
                .short('m')
                .long("md5")
                .help("Generate md5 digest for output file"),
        )
        .arg(
            Arg::new("tabix")
                .short('x')
                .long("tabix")
                .help("Generate tabix (tbx) index for output file"),
        )
        .arg(
            Arg::new("input")
                .takes_value(true)
                .value_name("PATH")
                .multiple_values(false)
                .help("Input VCF/BCF file"),
        )
        .arg(
            Arg::new("regions")
                .takes_value(true)
                .value_name("REGIONS")
                .multiple_values(true)
                .require_value_delimiter(true)
                .help("Chromosome regions (comma separated)"),
        )
}

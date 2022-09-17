use clap::{crate_version, App, Arg, ArgGroup, Command};

pub(super) fn cli_model() -> App<'static> {
    Command::new("mextr")
        .version(crate_version!())
        .author("Simon Heath <simon.heath@gmail.com>")
        .about("mextr is an application for extracting methylation information from VCF/BCF files")
        .next_help_heading("filtering (for gemBS bed output)")
        .arg(
            Arg::new("prop")
                .short('p')
                .long("prop")
                .takes_value(true)
                .value_name("FLOAT")
                .default_value("0.0")
                .help("Minimum proportion of sites/CpGs that must pass filters"),
        )
        .arg(
            Arg::new("number")
                .short('N')
                .long("prop")
                .takes_value(true)
                .value_name("INT")
                .default_value("1")
                .help("Minimum number of sites/CpGs that must pass filters"),
        )
        .arg(
            Arg::new("inform")
                .short('I')
                .long("inform")
                .takes_value(true)
                .value_name("INT")
                .default_value("1")
                .help("Minimum number of informative reads for a site/CpG"),
        )
        .arg(
            Arg::new("threshold")
                .short('T')
                .long("threshold")
                .takes_value(true)
                .value_name("INT")
                .default_value("20")
                .help("Minimum genotype PHRED score for a site/CpG"),
        )
        .arg(
            Arg::new("bq_threshold")
                .short('Q')
                .long("bq-threshold")
                .takes_value(true)
                .value_name("INT")
                .default_value("20")
                .help("Minimum base quality score for genotype calling"),
        )
        .arg(
            Arg::new("select")
                .short('s')
                .long("select")
                .takes_value(true)
                .value_name("MODE")
                .possible_values(&["hom", "het"])
                .default_value("hom")
                .ignore_case(true)
                .help("Minimum number of sites/CpGs that must pass filters"),
        )
        .arg(
            Arg::new("min_nc")
                .short('M')
                .long("min-nc")
                .takes_value(true)
                .value_name("INT")
                .default_value("1")
                .help("Minimum number of non-converted bases for non CpG site"),
        )
        .arg(
            Arg::new("report_file")
                .short('S')
                .long("report-filecpgfile")
                .takes_value(true)
                .value_name("PATH")
                .help("File with list of SNPs to be selected [default: select all passing sites]"),
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
        .next_help_heading("genotype recalling")
        .arg(
            Arg::new("reference_bias")
                .short('B')
                .long("reference-bias")
                .takes_value(true)
                .value_name("FLOAT")
                .default_value("2.0")
                .help("Reference bias for genotype calling"),
        )
        .arg(
            Arg::new("conversion")
                .short('c')
                .long("conversion")
                .takes_value(true)
                .value_name("FLOAT,FLOAT")
                .default_values(&["0.01", "0.05"])
                .multiple_values(true)
                .number_of_values(2)
                .use_value_delimiter(true)
                .help("Reference bias for genotype calling"),
        )
        .arg(
            Arg::new("common_gt")
                .short('g')
                .long("common-gt")
                .help("Assume common genotypes across all samples"),
        )
        .arg(
            Arg::new("haploid")
                .short('h')
                .long("haploid")
                .help("Force genotype calls to be haploid"),
        )
        .next_help_heading("output")
        .arg(
            Arg::new("cpgfile")
                .short('o')
                .long("cpgfile")
                .takes_value(true)
                .value_name("PATH")
                .help("File name for CpG file"),
        )
        .arg(
            Arg::new("noncpgfile")
                .short('n')
                .long("noncpgfile")
                .takes_value(true)
                .value_name("PATH")
                .help("File name for non CpG file"),
        )
        .arg(
            Arg::new("no_header")
                .short('H')
                .long("no-header")
                .help("Do not output header line for gemBS bed files"),
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .takes_value(true)
                .value_name("MODE")
                .possible_values(&["combined", "strand-specific"])
                .ignore_case(true)
                .default_value("combined")
                .help("Output mode for CpGs (gemBS bed)"),
        )
        .arg(
            Arg::new("bw_mode")
                .short('w')
                .long("bw-mode")
                .takes_value(true)
                .value_name("MODE")
                .possible_values(&["combined", "strand-specific"])
                .ignore_case(true)
                .default_value("combined")
                .help("Output mode for bigWig files"),
        )
        .arg(
            Arg::new("bed_methyl")
                .short('b')
                .long("bedmethyl")
                .alias("bed-methyl")
                .takes_value(true)
                .value_name("STRING")
                .help("File base name for bedmethyl files (not compatible with multisample VCFs/BCFs)"),
        )
        .arg(
            Arg::new("bed_track_line")
                .short('t')
                .long("bed-track-line")
                .takes_value(true)
                .value_name("STRING")
                .help("Track line for bedMethyl files [default: info taken from input file]"),
        )
        .arg(
            Arg::new("compress")
                .short('z')
                .long("compress")
                .help("Compress output file with bgzip"),
        )
        .arg(
            Arg::new("md5")
                .short('D')
                .long("md5")
                .help("Generate md5 digest for output file"),
        )
        .arg(
            Arg::new("tabix")
                .short('x')
                .long("tabix")
                .help("Generate tabix (tbx) index for output file"),
        )
        .next_help_heading("general")
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Silence all output"),
        )
        .arg(
            Arg::new("timestamp")
                .short('X')
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

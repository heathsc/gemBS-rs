use clap::{crate_version, App, Arg, Command};

pub(super) fn cli_model() -> App<'static> {
    Command::new("bs_call")
        .version(crate_version!())
        .author("Simon Heath <simon.heath@gmail.com>")
        .about("bs_call is an application for calling methylation values and SNV from bisulfite treated sequence data")
        .next_help_heading("filtering")
        .arg(
            Arg::new("keep_duplicates")
                .short('d')
                .long("keep-duplicates")
                .help("Don't merge duplicate reads"),
        )
        .arg(
            Arg::new("keep_supplementary")
                .short('S')
                .long("keep-supplementary")
                .help("Don't discard supplementary (split) alignments"),
        )
        .arg(
            Arg::new("ignore_duplicates")
                .short('e')
                .long("ignore-duplicates")
                .help("Ignore duplicate record flag in SAM record"),
        )
        .arg(
            Arg::new("keep_unmatched")
                .short('k')
                .long("keep-unmatched")
                .help("Don't discard reads that don't form proper pairs"),
        )
        .arg(
            Arg::new("right_trim")
                .short('R')
                .long("right-trim")
                .takes_value(true)
                .value_name("INT")
                .multiple_values(true)
                .require_value_delimiter(true)
                .max_values(2)
                .help("Bases to trim from right of read pair (give 2 values for read specific values)"),
        )
        .arg(
            Arg::new("left_trim")
                .short('L')
                .long("left-trim")
                .takes_value(true)
                .value_name("INT")
                .multiple_values(true)
                .require_value_delimiter(true)
                .max_values(2)
                .help("Bases to trim from left of read pair (give 2 values for read specific values)"),
        )
        .arg(
            Arg::new("blank_trim")
                .short('B')
                .long("blank-trim")
                .help("Don't use trimmed bases for genotype extimation"),
        )
        .arg(
            Arg::new("mapq_threshold")
                .short('q')
                .long("mapq-threshold")
                .takes_value(true)
                .value_name("INT")
                .default_value("20")
                .help("Minimum threshold for read MAPQ values"),
        )
        .arg(
            Arg::new("bq_threshold")
                .short('Q')
                .long("bq-threshold")
                .takes_value(true)
                .value_name("INT")
                .default_value("13")
                .help("Minimum threshold for base qualities"),
        )
        .arg(
            Arg::new("max_template_length")
                .short('l')
                .long("max-template-length")
                .takes_value(true)
                .value_name("INT")
                .default_value("1000")
                .help("Maximum threshold for paired read template length"),
        )
        .arg(
            Arg::new("contig_bed")
                .short('C')
                .long("contig-bed")
                .takes_value(true)
                .value_name("PATH")
                .help("BED file with contigs to be processed"),
        )
        .arg(
            Arg::new("contig_include")
                .short('s')
                .long("contig-include")
                .takes_value(true)
                .value_name("PATH")
                .help("File with list of contigs to be included"),
        )
        .arg(
            Arg::new("contig_exclude")
                .short('x')
                .long("contig-exclude")
                .takes_value(true)
                .value_name("PATH")
                .help("File with list of contigs to be excluded"),
        )
        .arg(
            Arg::new("filter_contigs")
                .short('F')
                .long("filter_contigs")
                .help("Only include in the outupt header contigs that are present in the contig_bed file")
                .requires("contig_bed")
        )
        .next_help_heading("calling")
        .arg(
            Arg::new("reference_bias")
                .short('R')
                .long("reference-bias")
                .takes_value(true)
                .value_name("FLOAT")
                .default_value("2.0")
                .help("Set weight on reference homozygote"),
        )
        .arg(
            Arg::new("conversion")
                .short('c')
                .long("conversion")
                .takes_value(true)
                .value_name("FLOAT")
                .multiple_values(true)
                .max_values(2)
                .require_value_delimiter(true)
                .default_values(&["0.01","0.05"])
                .help("Set under and over conversion rates"),
        )
        .arg(
            Arg::new("haploid")
                .short('1')
                .long("haploid")
                .help("Force genotype calls to be haploid")
        )
        .next_help_heading("output")
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .takes_value(true)
                .value_name("PATH")
                .help("Set output file name"),
        )
        .arg(
            Arg::new("output_type")
                .short('O')
                .long("output-type")
                .takes_value(true)
                .value_name("TYPE")
                .possible_values(&["b", "u", "z", "v"])
                .ignore_case(true)
                .help("Set output type - b: compressed BCF (default); u: uncompressed BCF; v: uncompressed VCF, z: compressed VCF"),
        )
        .arg(
            Arg::new("report_file")
                .short('j')
                .long("report-file")
                .takes_value(true)
                .value_name("PATH")
                .help("File name for Output JSON with calling statistics"),
        )
        .arg(
            Arg::new("sample")
                .short('n')
                .long("sample")
                .takes_value(true)
                .value_name("STRING")
                .help("Set sample name"),
        )
        .arg(
            Arg::new("all_positions")
                .short('A')
                .long("all-positions")
                .help("Output all covered positions")
        )
        .arg(
            Arg::new("benchmark_mode")
                .short('b')
                .long("benchmark-mode")
                .help("Benchmark mode (omits date / time / version numbers from output)")
        )
        .next_help_heading("references")
        .arg(
            Arg::new("reference")
                .short('r')
                .long("reference")
                .takes_value(true)
                .value_name("PATH")
                .required(true)
                .help("Path to genomic reference (FASTA)"),
        )
        .arg(
            Arg::new("dbsnp")
                .short('D')
                .long("dbsnp")
                .takes_value(true)
                .value_name("PATH")
                .help("Path to dbsnp_index processed index file (to add known SNP names)"),
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
            Arg::new("input")
                .takes_value(true)
                .value_name("PATH")
                .multiple_values(false)
                .help("Input SAM/BAM/CRAM file"),
        )
}

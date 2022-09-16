use clap::{crate_version, App, Arg, ArgGroup, Command};

pub(super) fn cli_model() -> App<'static> {
    Command::new("gemBS").version(crate_version!())
        .author("Simon Heath <simon.heath@gmail.com>")
        .about("gemBS is a bioinformatics pipeline for analyzing whole genome bisulfite data (WGBS) data")
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("quiet")
                .short('q').long("quiet")
                .help("Silence all output")
        )
        .arg(
            Arg::new("timestamp")
                .short('t').long("timestamp")
                .takes_value(true).value_name("GRANULARITY")
                .possible_values(&["none", "sec", "ms", "us", "ns"])
                .default_value("none")
                .help("Prepend log entries with a timestamp")
        )
        .arg(
            Arg::new("loglevel")
                .short('l').long("loglevel")
                .takes_value(true).value_name("LOGLEVEL")
                .possible_values(&["none", "error", "warn", "info", "debug", "trace"])
                .ignore_case(true)
                .default_value("warn")
                .help("Set log level")
        )
        .arg(
            Arg::new("config_file")
                .short('c').long("config-file")
                .takes_value(true).value_name("CONFIG_FILE")
                .help("Location of gemBS config file")
        )
        .arg(
            Arg::new("dir")
                .short('d').long("dir")
                .takes_value(true).value_name("DIR")
                .help("Set working directory")
        )
        .arg(
            Arg::new("gembs_root")
                .short('r').long("gembs-root")
                .takes_value(true).value_name("CONFIG_FILE")
                .help("Set root directory of gemBS installation")
        )
        .arg(
            Arg::new("all")
                .short('a').long("all")
                .help("Consider all tasks required for the requested command")
        )
        .arg(
            Arg::new("dry_run")
                .short('d').long("dry-run")
                .help("Output pending commands without execution")
        )
        .arg(
            Arg::new("json")
                .short('j').long("json")
                .takes_value(true).value_name("FILE")
                .help("Output JSON file with details of pending commands")
        )
        .arg(
            Arg::new("ignore_times")
                .short('i').long("ignore-times")
                .help("Ignore file modification times when evaluating the status of tasks")
        )
        .arg(
            Arg::new("ignore_status")
                .short('I').long("ignore-status")
                .help("Ignore status of tasks when compiling task list")
        )
        .arg(
            Arg::new("keep_logs")
                .short('k').long("keep-logs")
                .help("Don't remove log files after successful completion of task")
        )
        .subcommand(
            Command::new("prepare")
                .about("Prepare gemBS analysis")
                .arg(
                    Arg::new("config")
                        .short('c').long("config")
                        .takes_value(true).value_name("FILE")
                        .required(true)
                        .help("Text config file with gemBS parameters")
                )
                .arg(
                    Arg::new("cvs_metadata")
                        .short('t').long("text-metadata")
                        .takes_value(true).value_name("FILE")
                        .help("Sample metadata in csv format")
                )
                .arg(
                    Arg::new("json_metadata")
                        .short('l').long("json-metadata")
                        .takes_value(true).value_name("FILE")
                        .help("Sample metadata in JSON format")
                )
                .arg(
                    Arg::new("populate")
                        .short('p').long("populate-cache")
                        .takes_value(true).value_name("FILE")
                        .help("Populate local reference cache if required (for CRAM)")
                )
                .group(
                    ArgGroup::new("metadata")
                        .args(&["cvs_metadata, json_metadata"])
                        .required(true)
                )
        )
        .subcommand(
            Command::new("index")
                .about("Prepare genome indexes")
                .next_help_heading("index generation")
                .arg(
                    Arg::new("make_bs_index")
                        .short('b').long("bs-index")
                        .help("Generate bisulfite index")
                )
                .arg(
                    Arg::new("make_nonbs_index")
                        .short('n').long("nonbs-index")
                        .help("Generate non-bisulfite (regular) index")
                )
                .arg(
                    Arg::new("sampling")
                        .short('s').long("sampling-rate")
                        .hide(true)
                        .takes_value(true).value_name("INT")
                        .help("Text sampling rate - increasing will decrease index size but also performance")
                )
                .next_help_heading("dbSNP index options")
                .arg(
                    Arg::new("make_dbsnp_index")
                        .short('D').long("dbsnp-index")
                        .help("Generate dbSNP index")
                )
                .arg(
                    Arg::new("min_contig_size")
                        .short('M').long("min-contig-size")
                        .takes_value(true).value_name("INT")
                        .help("Contigs smaller than this will be filtered out during index generation")
                )
                .arg(
                    Arg::new("dbsnp_jobs")
                        .short('j').long("dbsnp-jobs")
                        .takes_value(true).value_name("INT")
                        .help("Number of parallel read jobs for dbsnp_index")
                )
                .arg(
                    Arg::new("dbsnp_files")
                        .short('d').long("dbsnp-files")
                        .takes_value(true).value_name("FILES")
                        .help("List of input files for dbSNP")
                )
                .arg(
                    Arg::new("dbsnp_selected")
                        .short('S').long("dbsnp-selected")
                        .takes_value(true).value_name("FILE")
                        .help("File with list of selected SNPs from dbSNP")
                )
                .arg(
                    Arg::new("dbsnp_type")
                        .long("dbsnp-type")
                        .takes_value(true).value_name("TYPE")
                        .possible_values(&["auto", "bed", "json", "vcf"])
                        .ignore_case(true)
                        .default_value("auto")
                        .help("Type of dbSNP input files")
                )
                .arg(
                    Arg::new("dbsnp_chrom_alias")
                        .long("dbsnp-chrom-alias")
                        .takes_value(true).value_name("FILE")
                        .help("File with list of chromosome aliases")
                )
                .next_help_heading("process options")
                .arg(
                    Arg::new("threads")
                        .short('t').long("threads")
                        .takes_value(true).value_name("INT")
                        .help("Number of threads [default: all available cores]")
                )
                .arg(
                    Arg::new("cores")
                        .short('c').long("cores")
                        .takes_value(true).value_name("INT")
                        .help("Number of cores [default: all available cores]")
                )
                .arg(
                    Arg::new("memory")
                        .short('m').long("memory")
                        .takes_value(true).value_name("INT")
                        .help("Memory required [default: all available memory]")
                )
                .arg(
                    Arg::new("time")
                        .short('T').long("time")
                        .takes_value(true).value_name("TIME")
                        .help("Time required")
                )
        )
        .subcommand(
            Command::new("map")
                .about("Read Mapping")
                .next_help_heading("dataset selection")
                .arg(
                    Arg::new("dataset")
                        .short('D').long("dataset")
                        .takes_value(true).value_name("STRING")
                        .help("Dataset to be mapped")
                )
                .arg(
                    Arg::new("sample")
                        .short('n').long("sample")
                        .takes_value(true).value_name("STRING")
                        .help("Name of sample to be mapped")
                )
                .arg(
                    Arg::new("barcode")
                        .short('b').long("barcode")
                        .takes_value(true).value_name("STRING")
                        .help("Barcode of sample to be mapped")
                )
                .group(
                    ArgGroup::new("sample_desc")
                        .args(&["dataset", "sample", "barcode"])
                )
                .next_help_heading("global options")
                .arg(
                    Arg::new("threads")
                        .short('t').long("threads")
                        .takes_value(true).value_name("INT")
                        .help("Number of threads for mapping pipeline [default: all available cores]")
                )
                .arg(
                    Arg::new("cores")
                        .short('c').long("cores")
                        .takes_value(true).value_name("INT")
                        .help("Number of cores [default: all available cores]")
                )
                .arg(
                    Arg::new("memory")
                        .short('m').long("memory")
                        .takes_value(true).value_name("INT")
                        .help("Memory required [default: all available memory]")
                )
                .arg(
                    Arg::new("time")
                        .short('T').long("time")
                        .takes_value(true).value_name("TIME")
                        .help("Time required")
                )
                .arg(
                    Arg::new("benchmark_mode")
                        .long("benchmark-mode")
                        .help("Omit dates etc. from output to make comparison simpler")
                )
                .next_help_heading("mapping options")
                .arg(
                    Arg::new("map_threads")
                        .long("map-threads")
                        .takes_value(true).value_name("INT")
                        .help("Number of threads for GEM3 mapper [default: threads]")
                )
                .arg(
                    Arg::new("underconv_seq")
                        .short('u').long("underconversion-sequence")
                        .takes_value(true).value_name("STRING")
                        .help("Contig name of underconversion sequencing control")
                )
                .arg(
                    Arg::new("overconv_seq")
                        .short('u').long("overconversion-sequence")
                        .takes_value(true).value_name("STRING")
                        .help("Contig name of overconversion sequencing control")
                )
                .arg(
                    Arg::new("file_type")
                        .short('F').long("file-type")
                        .takes_value(true).value_name("FILE_TYPE")
                        .possible_values(&["PAIRED", "SINGLE", "INTERLEAVED", "STREAM", "BAM", "CRAM"])
                        .ignore_case(true)
                        .help("Type of data file(s)")
                )
                .arg(
                    Arg::new("paired")
                        .short('p').long("paired-end")
                        .help("Input data is paired end")
                )
                .arg(
                    Arg::new("reverse")
                        .short('R').long("reverse-conversion")
                        .help("Assume G2A conversion on read 1 and C2T on read 2")
                )
                .arg(
                    Arg::new("non_stranded")
                        .short('s').long("read-non-stranded")
                        .help("Treat library as non stranded")
                )
                .arg(
                    Arg::new("bs")
                        .long("bs")
                        .help("Map as bisulfite data")
                )
                .arg(
                    Arg::new("non_bs")
                        .long("non-bs")
                        .help("Map as regular (non-bisulfite) data")
                )
                .group(
                    ArgGroup::new("mapping")
                        .args(&["bs", "non_bs"])
                )
                .next_help_heading("sorting options")
                .arg(
                    Arg::new("sort_threads")
                        .long("sort-threads")
                        .takes_value(true).value_name("INT")
                        .help("Number of threads for sorting [default: threads]")
                )
                .arg(
                    Arg::new("sort_memory")
                        .long("sort-memory")
                        .takes_value(true).value_name("INT")
                        .help("Memory allocated per sort threads")
                )
                .arg(
                    Arg::new("tmp_dir")
                        .short('d').long("tmp-dir")
                        .takes_value(true).value_name("PATH")
                        .help("Directory for temporary files (for sorting)")
                )
                .next_help_heading("merge options")
                .arg(
                    Arg::new("merge_threads")
                        .long("merge-threads")
                        .takes_value(true).value_name("INT")
                        .help("Number of threads for merging [default: threads]")
                )
                .arg(
                    Arg::new("merge")
                        .long("merge")
                        .help("Perform merge BAM step only")
                )
                .arg(
                    Arg::new("no_merge")
                        .long("no-merge")
                        .help("Do not automatically merge BAMs")
                )
                .arg(
                    Arg::new("md5")
                        .long("md5")
                        .help("Perform calculation of md5 sums only")
                )
                .arg(
                    Arg::new("no_md5")
                        .long("no-md5")
                        .help("Do not automatically calculate md5 sums")
                )
                .arg(
                    Arg::new("remove")
                        .short('r').long("remove")
                        .help("Remove individual BAMs after merging")
                )
                .group(
                    ArgGroup::new("remove")
                        .args(&["remove", "no_merge"])
                )
                .group(
                    ArgGroup::new("merging")
                        .args(&["merge", "no_merge", "md5", "no_md5"])
                )
                .group(
                    ArgGroup::new("md5")
                        .args(&["dataset", "md5", "no_md5"])
                )
        )
        .subcommand(
            Command::new("call")
                .about("Methylation and SNP calling")
                .next_help_heading("sample / contig selection")
                .arg(
                    Arg::new("sample")
                        .short('n').long("sample")
                        .takes_value(true).value_name("STRING")
                        .help("Name of sample to be processed")
                )
                .arg(
                    Arg::new("barcode")
                        .short('b').long("barcode")
                        .takes_value(true).value_name("STRING")
                        .help("Barcode of sample to be processed")
                )
                .arg(
                    Arg::new("pool")
                        .long("pool")
                        .takes_value(true).value_name("STRING")
                        .multiple_occurrences(true)
                        .help("Contig pool to be processed")
                )
                .group(
                    ArgGroup::new("sample_desc")
                        .args(&["sample", "barcode"])
                )
                .next_help_heading("filtering")
                .arg(
                    Arg::new("mapq_threshold")
                        .short('q').long("mapq-threshold")
                        .takes_value(true).value_name("INT")
                        .help("Minimum threshold for MAPQ scores")
                )
                .arg(
                    Arg::new("qual_threshold")
                        .short('q').long("qual-threshold")
                        .takes_value(true).value_name("INT")
                        .help("Minimum threshold for base quality scores")
                )
                .arg(
                    Arg::new("max_template_length")
                        .short('L').long("max-template-length")
                        .takes_value(true).value_name("INT")
                        .help("Maximum threshold for template length of paired reads")
                )
                .arg(
                    Arg::new("right_trim")
                        .short('g').long("right-trim")
                        .takes_value(true).value_name("INT [,INT]")
                        .multiple_values(true).require_value_delimiter(true)
                        .min_values(1).max_values(2)
                        .help("Bases to trim from right of read pair (give 2 values for read specific values)")
                )
                .arg(
                    Arg::new("left_trim")
                        .short('f').long("left-trim")
                        .takes_value(true).value_name("INT [,INT]")
                        .multiple_values(true).require_value_delimiter(true)
                        .min_values(1).max_values(2)
                        .help("Bases to trim from left of read pair (give 2 values for read specific values)")
                )
                .arg(
                    Arg::new("keep_duplicates")
                        .short('u').long("keep-duplicates")
                        .help("Do not merge duplicate reads")
                )
                .arg(
                    Arg::new("ignore_duplicate_flag")
                        .short('U').long("ignore-duplicate-flag")
                        .help("Ignore duplicate flag from input SAM/BAM/CRAM file")
                )
                .arg(
                    Arg::new("keep_unmatched")
                        .short('k').long("keep-unmatched")
                        .help("Do not discard reads that do not form proper pairs")
                )
                .next_help_heading("process options")
                .arg(
                    Arg::new("jobs")
                        .short('j').long("jobs")
                        .takes_value(true).value_name("INT")
                        .help("Number of parallel jobs")
                )
                .arg(
                    Arg::new("threads")
                        .short('t').long("threads")
                        .takes_value(true).value_name("INT")
                        .help("Number of threads for calling pipeline")
                )
                .arg(
                    Arg::new("call_threads")
                        .long("threads")
                        .takes_value(true).value_name("INT")
                        .help("Number of threads for methylation caller")
                )
                .arg(
                    Arg::new("merge_threads")
                        .long("merge-threads")
                        .takes_value(true).value_name("INT")
                        .help("Number of threads for BCF merging step")
                )
                .arg(
                    Arg::new("cores")
                        .short('c').long("cores")
                        .takes_value(true).value_name("INT")
                        .help("Number of cores for a job [default: available cores / jobs]")
                )
                .arg(
                    Arg::new("memory")
                        .short('m').long("memory")
                        .takes_value(true).value_name("INT")
                        .help("Memory required for a job [default: available memory / jobs]")
                )
                .arg(
                    Arg::new("time")
                        .short('T').long("time")
                        .takes_value(true).value_name("TIME")
                        .help("Time required for a job")
                )
                .next_help_heading("calling options")
                .arg(
                    Arg::new("haploid")
                        .short('1').long("haploid")
                        .help("Force genotype calls to be haploid")
                )
                .arg(
                    Arg::new("conversion")
                        .short('C').long("conversion")
                        .takes_value(true).value_name("INT INT")
                        .multiple_values(true).number_of_values(2)
                        .help("Set conversion rates (under over)")
                )
                .arg(
                    Arg::new("auto_conversion")
                        .long("auto-conversion")
                        .help("Estimate conversion rates from mapping data")
                )
                .group(
                    ArgGroup::new("conversion")
                        .args(&["conversion", "auto_conversion"])
                )
                .arg(
                    Arg::new("ref_bias")
                        .short('R').long("reference-bias")
                        .takes_value(true).value_name("FLOAT")
                        .help("Set bias (weight) for reference homozygote")
                )
                .arg(
                    Arg::new("dbsnp_index")
                        .short('D').long("dbsnp-index")
                        .takes_value(true).value_name("PATH")
                        .help("Path to dbSNP_idx processed SNP database file")
                )
                .next_help_heading("general options")
                .arg(
                    Arg::new("md5")
                        .long("md5")
                        .help("Perform calculation of md5 sums only")
                )
                .arg(
                    Arg::new("no_md5")
                        .long("no-md5")
                        .help("Do not automatically calculate md5 sums")
                )
                .group(
                    ArgGroup::new("md5")
                        .args(&["md5", "no_md5"])
                )
                .arg(
                    Arg::new("index")
                        .long("index")
                        .help("Perform indexing of final BCF only")
                )
                .arg(
                    Arg::new("no_index")
                        .long("no-index")
                        .help("Do not automatically calculate index final BCF")
                )
                .group(
                    ArgGroup::new("index")
                        .args(&["index", "no_index"])
                )
                .arg(
                    Arg::new("remove")
                        .short('r').long("remove")
                        .help("Remove individual BCFs after merging")
                )
                .group(
                    ArgGroup::new("remove")
                        .args(&["remove", "no_merge"])
                )
                .arg(
                    Arg::new("merge")
                        .long("merge")
                        .help("Perform merge BCF step only")
                )
                .arg(
                    Arg::new("no_merge")
                        .long("no-merge")
                        .help("Do not automatically merge BCFs")
                )
                .group(
                    ArgGroup::new("merge")
                        .args(&["merge", "no_merge", "md5"])
                )
                .arg(
                    Arg::new("species")
                        .short('e').long("species")
                        .takes_value(true).value_name("STRING")
                        .help("Species name")
                )
                .arg(
                    Arg::new("benchmark_mode")
                        .long("benchmark-mode")
                        .help("Omit dates etc. from output to make comparison simpler")
                )
        )
        .subcommand(
            Command::new("extract")
                .about("Produce methylation and SNP summary files")
                .next_help_heading("sample / contig selection")
                .arg(
                    Arg::new("sample")
                        .short('n').long("sample")
                        .takes_value(true).value_name("STRING")
                        .help("Name of sample to be processed")
                )
                .arg(
                    Arg::new("barcode")
                        .short('b').long("barcode")
                        .takes_value(true).value_name("STRING")
                        .help("Barcode of sample to be processed")
                )
                .group(
                    ArgGroup::new("sample_desc")
                        .args(&["sample", "barcode"])
                )
                .next_help_heading("filtering")
                .arg(
                    Arg::new("qual_threshold")
                        .short('Q').long("qual-threshold")
                        .takes_value(true).value_name("INT")
                        .help("Minimum threshold for base quality scores")
                )
                .arg(
                    Arg::new("phred_threshold")
                        .short('q').long("phred-threshold")
                        .takes_value(true).value_name("INT")
                        .help("Minimum threshold for genotype PHRED score (gemBS bed)")
                )
                .arg(
                    Arg::new("min_inform")
                        .short('I').long("min-inform")
                        .takes_value(true).value_name("INT")
                        .help("Minimum threshold for informative reads (gemBS bed)")
                )
                .arg(
                    Arg::new("min_nc")
                        .short('I').long("min-nc")
                        .takes_value(true).value_name("INT")
                        .help("Minimum threshold for non-converted reads for non-CpG sites (gemBS bed)")
                )
                .arg(
                    Arg::new("allow_het")
                        .short('H').long("allow-het")
                        .takes_value(true).value_name("INT")
                        .help("Allow both homozygous and heterozygous sites (gemBS bed)")
                )
                .arg(
                    Arg::new("ref_bias")
                        .short('R').long("reference-bias")
                        .takes_value(true).value_name("FLOAT")
                        .help("Set bias (weight) for reference homozygote")
                )
                .next_help_heading("output options")
                .arg(
                    Arg::new("cpg")
                        .short('C').long("cpg")
                        .help("Output gemBS bed with filtered CpG sites")
                )
                .arg(
                    Arg::new("non_cpg")
                        .short('N').long("non-cpg")
                        .help("Output gemBS bed with filtered non-CpG sites")
                )
                .arg(
                    Arg::new("strand_specific")
                        .short('s').long("strand-specific")
                        .help("Output separate lines in CpG file for each strand (gemBS bed)")
                )
                .arg(
                    Arg::new("bed_methyl")
                        .short('B').long("bedmethyl").alias("bed-methyl")
                        .help("Output ENCODE standard output (bedMethyl, bigBed and bigWig)")
                )
                .arg(
                    Arg::new("bigwig_strand_specific")
                        .short('W').long("bigwig-strand-specific")
                        .help("Output separate bigWig files for each strand (bedMethyl output)")
                )
                .arg(
                    Arg::new("snps")
                        .short('S').long("snps")
                        .help("Output snps")
                )
                .arg(
                    Arg::new("snp_list")
                        .long("snp-list")
                        .takes_value(true).value_name("PATH")
                        .help("Path to file with list of SNPs to output")
                )
                .arg(
                    Arg::new("snp_db")
                        .long("snp-db")
                        .takes_value(true).value_name("PATH")
                        .help("Path to dbSNP_idx processed SNP database file")
                )
                .arg(
                    Arg::new("no_md5")
                        .long("no-md5")
                        .help("Do not automatically calculate md5 sums")
                )
                .next_help_heading("process options")
                .arg(
                    Arg::new("jobs")
                        .short('j').long("jobs")
                        .takes_value(true).value_name("INT")
                        .help("Number of parallel jobs")
                )
                .arg(
                    Arg::new("threads")
                        .short('t').long("threads")
                        .takes_value(true).value_name("INT")
                        .help("Number of threads for extraction pipeline")
                )
                .arg(
                    Arg::new("cores")
                        .short('c').long("cores")
                        .takes_value(true).value_name("INT")
                        .help("Number of cores for a job [default: available cores / jobs]")
                )
                .arg(
                    Arg::new("memory")
                        .short('m').long("memory")
                        .takes_value(true).value_name("INT")
                        .help("Memory required for a job [default: available memory / jobs]")
                )
                .arg(
                    Arg::new("time")
                        .short('T').long("time")
                        .takes_value(true).value_name("TIME")
                        .help("Time required for a job")
                )
        )
        .subcommand(
            Command::new("report")
                .about("Generate QC reports")
                .arg(
                    Arg::new("project")
                        .short('p').long("project")
                        .takes_value(true).value_name("STRING")
                        .help("Output title for report (project name)")
                )
                .arg(
                    Arg::new("report_dir")
                        .short('r').long("report-dir")
                        .takes_value(true).value_name("PATH")
                        .default_value("gemBS_reports")
                        .help("Root directory for reports")
                )
                .arg(
                    Arg::new("paper_size")
                        .short('S').long("paper-size")
                        .takes_value(true).value_name("PAPER_SIZE")
                        .possible_values(&["a4", "letter"])
                        .ignore_case(true)
                        .help("Select paper size of latex version of report")
                )
                .arg(
                    Arg::new("mapping")
                        .short('M').long("mapping")
                        .help("Generate the mapping report only")
                )
                .arg(
                    Arg::new("calling")
                        .short('C').long("calling")
                        .help("Generate the calling report only")
                )
                .arg(
                    Arg::new("pdf")
                        .short('P').long("pdf")
                        .help("Generate pdf version of report (requires latexmk and pdflatex)")
                )
                .arg(
                    Arg::new("threads")
                        .short('t').long("threads")
                        .takes_value(true).value_name("INT")
                        .help("Number of threads for report generation")
                )
                .arg(
                    Arg::new("cores")
                        .short('c').long("cores")
                        .takes_value(true).value_name("INT")
                        .help("Number of cores")
                )
                .arg(
                    Arg::new("memory")
                        .short('m').long("memory")
                        .takes_value(true).value_name("INT")
                        .help("Memory required")
                )
                .arg(
                    Arg::new("time")
                        .short('T').long("time")
                        .takes_value(true).value_name("TIME")
                        .help("Time required")
                )

        )
        .subcommand(
            Command::new("run")
                .about("Run all pending pipeline commands")
        )
        .subcommand(
            Command::new("completions")
                .about("Generate shell completion files")
                .arg(
                    Arg::new("shell")
                        .short('s').long("shell")
                        .takes_value(true).value_name("SHELL")
                        .possible_values(&["bash", "zsh", "fish", "elvish","powershell"])
                        .default_value("bash")
                        .help("Destination shell for completion file")
                )
                .arg(
                    Arg::new("output")
                        .short('o').long("output")
                        .takes_value(true).value_name("PATH")
                        .default_value("gemBS")
                        .help("Output file")
                )

        )
        .subcommand(
            Command::new("clear")
                .about("Clear up incomplete files after aborted run")
                .arg(
                    Arg::new("confirm")
                        .short('y').long("yes")
                        .help("Do not ask for confirmation")
                )
                .arg(
                    Arg::new("force")
                        .short('f').long("force")
                        .help("Continue even if lock file found")
                )
        )
}
use clap::{App, ArgGroup, ArgMatches};
use std::path::Path;
use std::{env, fs};
use utils::log_level::init_log;

#[cfg(feature = "slurm")]
use clap::Arg;
use clap_complete::{generate, Shell};

use crate::commands;
use crate::common::defs::{DataValue, Section};
use crate::config::GemBS;
use cli_model::cli_model;

use crate::cli::cli_model;

fn gen_cli() -> App<'static> {
    #[cfg(feature = "slurm")]
    {
        let container: Option<&'static str> = option_env!("GEMBS_CONTAINER");
        if container.is_none() {
            cli_model()
                .arg(
                    Arg::with_name("slurm_script")
                        .short('s')
                        .long("slurm-script")
                        .takes_value(true)
                        .value_name("SCRIPT_FILE")
                        .help("Generate PERL script to submit commands to slurm for execution"),
                )
                .arg(
                    Arg::with_name("slurm")
                        .short('S')
                        .long("slurm")
                        .help("Submit commands to slurm for execution"),
                )
                .group(ArgGroup::with_name("slurm_opts").args(&["slurm", "slurm_script"]))
        } else {
            cli_model().arg(
                Arg::with_name("slurm_script")
                    .short('s')
                    .long("slurm-script")
                    .takes_value(true)
                    .value_name("SCRIPT_FILE")
                    .help("Generate PERL script to submit commands to slurm for execution"),
            )
        }
    }
    #[cfg(not(feature = "slurm"))]
    {
        cli_model()
    }
}

fn generate_completions(m: &ArgMatches) -> Result<(), String> {
    let gen = m
        .get_one::<Shell>("shell")
        .copied()
        .ok_or("Unknown shell")?;
    let mut cmd = cli_model();
    eprintln!("Generating completion file for {}...", gen);
    let ofile = m.value_of("output").expect("Default output option missing");

    match fs::File::create(&ofile) {
        Ok(mut file) => {
            generate(gen, &mut cmd, "gemBS", &mut file);
            Ok(())
        }
        Err(e) => Err(format!(
            "Couldn't create shell completion file {}: {}",
            ofile, e
        )),
    }
}

pub fn process_cli(gem_bs: &mut GemBS) -> Result<(), String> {
    let m = gen_cli().get_matches();
    // Interpret global command line flags and set up logging

    let (verbose, _) = init_log(&m);
    gem_bs.set_verbose(verbose);
    if let Some(f) = m.value_of("dir") {
        let wd = Path::new(f);
        env::set_current_dir(&wd)
            .map_err(|e| format!("Can not switch working directory to {}: {}", f, e))?;
        debug!("Moved working directory to {}", f);
    }
    if let Some(s) = m.value_of("config_file") {
        gem_bs.set_config(
            Section::Default,
            "config_file",
            DataValue::String(s.to_string()),
        );
    }
    if let Some(s) = m.value_of("gembs_root") {
        gem_bs.set_config(
            Section::Default,
            "gembs_root",
            DataValue::String(s.to_string()),
        );
    }
    if m.is_present("keep_logs") {
        gem_bs.set_keep_logs(true)
    }
    if m.is_present("ignore_times") {
        gem_bs.set_ignore_times(true);
    }
    if m.is_present("ignore_status") {
        gem_bs.set_ignore_status(true);
    }
    if m.is_present("all") {
        gem_bs.set_all(true);
    }
    if m.is_present("dry_run") {
        gem_bs.set_dry_run(true);
    }
    if m.is_present("slurm") {
        gem_bs.set_slurm(true);
    }
    if let Some(s) = m.value_of("json") {
        gem_bs.set_json_out(s);
    }
    if let Some(s) = m.value_of("slurm_script") {
        gem_bs.set_slurm_script(s);
    }

    let mem = (gem_bs.total_mem() as f64) / 1073741824.0;
    debug!("Total memory detected: {:.1} GB", mem);

    // Now handle subcommands

    match m.subcommand() {
        Some(("prepare", m_sum)) => commands::prepare::prepare_command(m_sum, gem_bs),
        Some(("index", m_sum)) => commands::index::index_command(m_sum, gem_bs),
        Some(("map", m_sum)) => commands::map::map_command(m_sum, gem_bs),
        Some(("call", m_sum)) => commands::call::call_command(m_sum, gem_bs),
        Some(("extract", m_sum)) => commands::extract::extract_command(m_sum, gem_bs),
        Some(("report", m_sum)) => commands::report::report_command(m_sum, gem_bs),
        Some(("run", m_sum)) => commands::run::run_command(m_sum, gem_bs),
        Some(("clear", m_sum)) => commands::clear::clear_command(m_sum, gem_bs),
        Some(("completions", m_sum)) => generate_completions(m_sum),
        _ => Err("Unknown or missing subcommand".to_string()),
    }
}

use clap::{ArgAction, ArgGroup, ArgMatches};
use std::{env, fs, path::PathBuf};
use utils::log_level::init_log;

#[cfg(feature = "slurm")]
use clap::{Arg, Command};
use clap_complete::{generate, Shell};

use crate::commands;
use crate::common::defs::{DataValue, Section};
use crate::config::GemBS;
use cli_model::cli_model;

use crate::cli::cli_model;
use clap::value_parser;

fn gen_cli() -> Command {
    #[cfg(feature = "slurm")]
    {
        let container: Option<&'static str> = option_env!("GEMBS_CONTAINER");
        if container.is_none() {
            cli_model()
                .arg(
                    Arg::new("slurm_script")
                        .short('s')
                        .long("slurm-script")
                        .value_name("FILE")
                        .value_parser(value_parser!(String))
                        .help("Generate PERL script to submit commands to slurm for execution"),
                )
                .arg(
                    Arg::new("slurm")
                        .short('S')
                        .long("slurm")
                        .action(ArgAction::SetTrue)
                        .help("Submit commands to slurm for execution"),
                )
                .group(ArgGroup::new("slurm_opts").args(&["slurm", "slurm_script"]))
        } else {
            cli_model().arg(
                Arg::new("slurm_script")
                    .short('s')
                    .long("slurm-script")
                    .value_parser(value_parser!(String))
                    .value_name("FILE")
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
    //    Err("Completions currently not working".to_string())
    let gen = *m.get_one::<Shell>("shell").expect("Missing shell");
    let mut cmd = cli_model();
    eprintln!("Generating completion file for {}...", gen);
    let ofile = m
        .get_one::<String>("output")
        .expect("Default output option missing");

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
    if let Some(wd) = m.get_one::<PathBuf>("dir") {
        env::set_current_dir(wd).map_err(|e| {
            format!(
                "Can not switch working directory to {}: {}",
                wd.display(),
                e
            )
        })?;
        debug!("Moved working directory to {}", wd.display());
    }
    if let Some(s) = m.get_one::<String>("config_file") {
        gem_bs.set_config(
            Section::Default,
            "config_file",
            DataValue::String(s.clone()),
        );
    }
    if let Some(s) = m.get_one::<String>("gembs_root") {
        gem_bs.set_config(Section::Default, "gembs_root", DataValue::String(s.clone()));
    }
    gem_bs.set_keep_logs(m.get_flag("keep_logs"));
    gem_bs.set_ignore_times(m.get_flag("ignore_times"));
    gem_bs.set_ignore_status(m.get_flag("ignore_status"));
    gem_bs.set_all(m.get_flag("all"));
    gem_bs.set_dry_run(m.get_flag("dry_run"));
    gem_bs.set_slurm(m.get_flag("slurm"));

    if let Some(s) = m.get_one::<String>("json") {
        gem_bs.set_json_out(s);
    }
    if let Some(s) = m.get_one::<String>("slurm_script") {
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

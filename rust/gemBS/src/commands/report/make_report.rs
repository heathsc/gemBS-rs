use std::{
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicUsize,
    sync::Arc,
};

use chrono::prelude::*;

use super::make_map_report::{make_section, make_title};
use crate::common::html_utils::*;
use crate::common::latex_utils::*;
use crate::common::utils;
use crate::scheduler::report::ReportOptions;

fn create_html_report(path: &PathBuf, project: &str) -> Result<(), String> {
    let mut html = HtmlPage::new(&path)?;
    let mut head_element = HtmlElement::new("HEAD", None, true);
    let mut style_element = HtmlElement::new("STYLE", Some("TYPE=\"text/css\""), true);
    style_element.push_str("<!--\n@import url(\"css/style.css\");\n-->");
    head_element.push_element(style_element);
    html.push_element(head_element);
    let mut body = HtmlElement::new("BODY", None, true);
    body.push_element(make_title(format!(
        "GemBS QC & Analysis Pipeline Report: Project {}",
        project
    )));
    body.push_element(make_section("QC report from mapping stage using GEM3"));
    body.push_element(HtmlElement::new(
        "iframe",
        Some("id=\"frame\" src=\"mapping/index.html\""),
        true,
    ));
    body.push_element(make_section(
        "QC report for methylation & variant calling stage using bs_call",
    ));
    body.push_element(HtmlElement::new(
        "iframe",
        Some("id=\"frame\" src=\"calling/index.html\""),
        true,
    ));
    html.push_element(body);
    Ok(())
}

/// Create top level latex report file
fn create_latex_report(
    path: &PathBuf,
    project: &str,
    rep_opt: &ReportOptions,
) -> Result<(), String> {
    // Handle extra latex files
    let output_dir = path
        .parent()
        .expect("No parent directory found for map report");
    let extras = output_dir.join("extras");
    fs::create_dir_all(&extras)
        .map_err(|e| format!("Could not create directory for latex extras: {}", e))?;
    let origin = rep_opt.extras_path.as_path();

    // Copy template file
    let latex_template = rep_opt
        .latex_template
        .as_deref()
        .unwrap_or("gemBS_report_default.tex");

    debug!(
        "Copy template file {} to {}",
        latex_template,
        extras.display()
    );

    fs::copy(origin.join(latex_template), extras.join("gemBS_report.tex"))
        .map_err(|e| format!("Could not copy latex template {}: {}", latex_template, e))?;

    // Copy extra files
    let mut ex_files = vec!["sphinx.sty"];
    if let Some(v) = rep_opt.extra_latex_files.as_deref() {
        for f in v {
            ex_files.push(f.as_str())
        }
    } else {
        ex_files.push("gembsbook_default.cls")
    }
    for f in ex_files {
        debug!("Copy extra latex file {} to {}", f, extras.display());
        fs::copy(origin.join(f), extras.join(f))
            .map_err(|e| format!("Could not copy extra latex file {}: {}", f, e))?;
    }

    let sz = match rep_opt.page_size {
        PageSize::A4 => "a4paper",
        PageSize::Letter => "letter",
    };

    let nc = |c, s| format!("\\newcommand{{\\{c}}}{{{s}}}");

    let mut out = Vec::with_capacity(16);

    if let Some(s) = rep_opt.project.as_deref() {
        out.push(nc("projectname", s))
    }
    out.push(nc("reportpagesize", sz));
    if let Some(s) = rep_opt.collaborator_name.as_deref() {
        out.push(nc("collaborator", s))
    }
    if let Some(s) = rep_opt.collaborator_email.as_deref() {
        out.push(nc("collaboratoremail", s))
    }
    if let Some(s) = rep_opt.analyst_name.as_deref() {
        out.push(nc("analystname", s))
    }
    if let Some(s) = rep_opt.analyst_team.as_deref() {
        out.push(nc("analystteam", s))
    }
    if let Some(s) = rep_opt.analyst_email.as_deref() {
        out.push(nc("analystemail", s))
    }
    if let Some(s) = rep_opt.analysis_start_date.as_deref() {
        out.push(nc("startdate", s))
    }
    if let Some(s) = rep_opt.comment.as_deref() {
        out.push(nc("comment", s))
    }
    let finish_date = rep_opt
        .analysis_end_date
        .as_ref()
        .cloned()
        .unwrap_or_else(|| {
            let local = Local::now();
            format!("{}", local.format("%d-%m-%Y"))
        });
    out.push(nc("finishdate", finish_date.as_str()));

    out.push(format!(
        "\\title{{QC report from GemBS methylation pipeline for project {project}}}"
    ));
    out.push("\\author{GemBS}".to_string());

    if !rep_opt.samples.is_empty() {
        let mut samples = String::new();
        let mut first = true;
        for (s, _) in rep_opt.samples.iter() {
            if first {
                first = false
            } else {
                samples.push(',')
            }
            samples.push_str(s.as_str());
        }

        out.push(nc("samples", samples.as_str()))
    }

    out.push(format!("\\input{{extras/gemBS_report.tex}}"));

    // Write out top level latex file
    let ofile = fs::File::create(path)
        .map_err(|e| format!("Error opening latex output file {}: {}", path.display(), e))?;
    let mut writer = Box::new(BufWriter::new(ofile));

    for s in out {
        writeln!(writer, "{s}")
            .map_err(|e| format!("Error writing to latex file {}: {e}", path.display()))?
    }
    Ok(())
}

pub fn make_report(
    sig: Arc<AtomicUsize>,
    outputs: &[PathBuf],
    rep_opt: ReportOptions,
) -> Result<(), String> {
    utils::check_signal(Arc::clone(&sig))?;
    info!("Making summary report");
    let project = rep_opt.project.as_deref().unwrap_or("gemBS");
    let report_tex_path = outputs.get(0).expect("No output files for report");
    let report_html_path = outputs.get(1).expect("No html output file for report");
    let output_dir = report_tex_path
        .parent()
        .expect("No parent directory found for map report");
    create_html_report(report_html_path, project)?;
    create_latex_report(report_tex_path, project, &rep_opt)?;
    if rep_opt.pdf {
        info!("Making pdf version of summary report");
        let report_pdf_path = outputs.get(2).expect("No pdf output file for report");
        let pdf_name = Path::new(
            report_pdf_path
                .file_name()
                .expect("No file name for pdf output file"),
        );

        // Handle extra latex files
        let extras = output_dir.join("extras");
        fs::create_dir_all(&extras)
            .map_err(|e| format!("Could not create directory for latex extras: {}", e))?;
        let origin = rep_opt.extras_path.as_path();
        // Copy template file
        let latex_template = rep_opt
            .latex_template
            .as_deref()
            .unwrap_or("gemBS_report_default.tex");
        fs::copy(origin.join(latex_template), extras.join("gemBS_report.tex"))
            .map_err(|e| format!("Could not copy latex template: {}", e))?;

        let mut pipeline = utils::Pipeline::new();
        let tex_name = format!("{}", report_tex_path.display());
        let args = vec!["-pdf", "-silent", "-cd", "-outdir=.latexwork", &tex_name];
        let path = Path::new("latexmk");
        let ofile = output_dir.join("latexmk.log");
        pipeline
            .add_stage(&path, Some(args.iter()))
            .log_file(output_dir.join("latexmk.err"))
            .out_filepath(&ofile);
        let tdir = output_dir.join(".latexwork");
        // Older versions of latexmk need the output directory to exist before running
        fs::create_dir_all(&tdir).expect("Could not create temporary output directory for latexmk");
        match pipeline.run(Arc::clone(&sig)) {
            Ok(_) => {
                fs::rename(tdir.join(pdf_name), report_pdf_path)
                    .map_err(|e| format!("Could not find output pdf file: {}", e))?;
                fs::remove_dir_all(tdir)
                    .map_err(|e| format!("Could not remove latexmk work directory: {}", e))?;
                let _ = fs::remove_file(output_dir.join("latexmk.err"));
                let _ = fs::remove_file(ofile);
            }
            Err(e) => {
                let _ = fs::remove_dir_all(tdir);
                return Err(e);
            }
        }
    }
    Ok(())
}

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::fs;

use crate::common::utils;
use super::make_map_report::{make_title, make_section};
use crate::common::html_utils::*;
use crate::common::latex_utils::*;

fn create_html_report(path: &PathBuf, project: &str) -> Result<(), String> {
	let mut html = HtmlPage::new(&path)?;
	let mut head_element = HtmlElement::new("HEAD", None, true);
	let mut style_element = HtmlElement::new("STYLE", Some("TYPE=\"text/css\""), true);
	style_element.push_str("<!--\n@import url(\"css/style.css\");\n-->");
	head_element.push_element(style_element);
	html.push_element(head_element);
	let mut body = HtmlElement::new("BODY", None, true);
	body.push_element(make_title(format!("GemBS QC & Analysis Pipeline Report: Project {}", project)));
	body.push_element(make_section("QC report from mapping stage using GEM3"));
	body.push_element(HtmlElement::new("iframe", Some("id=\"frame\" src=\"mapping/index.html\""), true));
	body.push_element(make_section("QC report for methylation & variant calling stage using bs_call"));
	body.push_element(HtmlElement::new("iframe", Some("id=\"frame\" src=\"calling/index.html\""), true));
	html.push_element(body);	
	Ok(())
}

fn create_latex_report(path: &PathBuf, project: &str, page_size: PageSize) -> Result<(), String> {
	let mut doc = LatexDoc::new(&path, page_size, format!("QC report from GemBS methylation pipeline for project: {}", project).as_str(), "GemBS")?;
	doc.push(LatexContent::Text("\\chapter{Mapping using GemBS}".to_string()));
	doc.push(LatexContent::Text("\\input{mapping/map_report.tex}".to_string()));
	doc.push(LatexContent::Text("\\chapter{Methylation \\& variant calling using bs\\_call}".to_string()));
	doc.push(LatexContent::Text("\\input{calling/call_report.tex}".to_string()));
	Ok(())	
}

pub fn make_report(sig: Arc<AtomicUsize>, outputs: &[PathBuf], project: Option<String>, page_size: PageSize, pdf: bool) -> Result<(), String> {
	utils::check_signal(Arc::clone(&sig))?;
	let project = project.unwrap_or_else(|| "gemBS".to_string());
	let report_tex_path = outputs.get(0).expect("No output files for report");
	let report_html_path = outputs.get(1).expect("No html output file for report");
	let output_dir = report_tex_path.parent().expect("No parent directory found for map report");
	create_html_report(report_html_path, &project)?;
	create_latex_report(report_tex_path, &project, page_size)?;
	if pdf {	
		let report_pdf_path = outputs.get(2).expect("No pdf output file for report");
		let pdf_name = Path::new(report_pdf_path.file_name().expect("No file name for pdf output file"));
		let mut pipeline = utils::Pipeline::new();
		let tex_name = format!("{}", report_tex_path.display());
		let args = vec!("-pdf", "-silent", "-cd", "-outdir=.latexwork", &tex_name);
		let path = Path::new("latexmk");
		let ofile = output_dir.join("latexmk.log");
		pipeline.add_stage(&path, Some(args.iter())).log_file(output_dir.join("latexmk.err")).out_file(&ofile);
		let tdir = output_dir.join(".latexwork");
		match pipeline.run(Arc::clone(&sig)) {
			Ok(_) => {
				fs::rename(tdir.join(pdf_name), report_pdf_path).map_err(|e| format!("Could not find output pdf file: {}", e))?;
				fs::remove_dir_all(tdir).map_err(|e| format!("Could not remove latexmk work directory: {}", e))?;
				let _ = fs::remove_file(output_dir.join("latexmk.err"));
				let _ = fs::remove_file(ofile);
			},
			Err(e) => {
				let _ = fs::remove_dir_all(tdir);
				return Err(e);
			}
		}
	}
	Ok(())
}
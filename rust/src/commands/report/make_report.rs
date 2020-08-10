use std::path::PathBuf;
use std::io::BufRead;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

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

pub fn make_report(sig: Arc<AtomicUsize>, outputs: &[PathBuf], project: Option<String>, page_size: PageSize) -> Result<Option<Box<dyn BufRead>>, String> {
	utils::check_signal(Arc::clone(&sig))?;
	let project = project.unwrap_or_else(|| "gemBS".to_string());
	let report_tex_path = outputs.get(0).expect("No output files for report");
	let report_html_path = outputs.get(1).expect("No html output file for report");
	create_html_report(report_html_path, &project)?;
	create_latex_report(report_tex_path, &project, page_size)?;
	Ok(None)
}
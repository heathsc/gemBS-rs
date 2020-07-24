use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, mpsc, RwLock};
use std::{fs, thread, time};
use std::collections::HashMap;
use std::str::FromStr;

use plotters::prelude::*;

use crate::common::{utils, compress};
use crate::common::json_call_stats::{CallJson, Coverage, FSReadLevelType, FSBaseLevelType, FSCounts, Counts, QCCounts};
use crate::scheduler::report::CallJsonFiles;
use super::report_utils::*;
use super::make_map_report;
use super::make_map_report::{make_title, make_section};
use crate::common::html_utils::*;

enum CovType { All, NonRefCpg, NonRefCpgInf, RefCpg, RefCpgInf, Variant }
enum QualType { All, RefCpg, NonRefCpg, Variant }
enum QCDistType { QDVariant, QDNonVariant, RMSVariant, RMSNonVariant }

fn make_hist(path: &Path, ch: &HashMap<usize, usize>, title: &str, xlabel: &str, ylabel: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(&path, (640, 480)).into_drawing_area();
	root.fill(&WHITE)?;
	
	let mut total = 0;
	let mut m = 0;
	let mut t = Vec::new();
	for(x,y) in ch.iter() {
		total += y;
		if *y > m { m = *y; }
		t.push((*x,*y));
	}
	let lim_y = 0.99 * (total as f64);
	t.sort_by(|a,b| a.0.cmp(&b.0));
	let mut lim_x = t[0].0;
	total = 0;
	for(x, y) in t.iter() {
		total += y;
		if (total as f64) >= lim_y { break; }
		lim_x = *x; 
	}
	let max = (m + 1) as f64;
	let mx = (lim_x + 1) as f64;
    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(60)
        .margin(5)
        .caption(title, ("sans-serif", 22.0).into_font())
        .build_ranged(0.0..mx, 0.0..max)?;

    chart
        .configure_mesh()
        .line_style_1(&WHITE.mix(0.3))
        .y_desc(ylabel)
        .x_desc(xlabel)
		.y_label_formatter(&|y| format!("{:e}", y))
		.x_label_formatter(&|x| format!("{:.0}", x))
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;

    chart.draw_series(
		t.iter().map(|(x, y)| {
			Rectangle::new([((*x as f64) - 0.5, 0.0), ((*x as f64) + 0.5, (*y as f64))], BLUE.mix(0.4).filled())
		})
    )?;
	Ok(())
}

fn make_coverage_graph(bc: &str, dir: &Path, cov: CovType, call_json: &CallJson) -> Result<(), String> {
	let coverage = call_json.coverage();
	let (name, title, xaxis, yaxis, ch) = match cov {
		CovType::All => ("all", "Coverage at All Sites", "Coverage", "# sites", &coverage.all ),
		CovType::RefCpg =>("ref_cpg", "Coverage at Reference CpGs", "Coverage", "# CpGs", &coverage.ref_cpg ),
		CovType::RefCpgInf => ("ref_cpg_inf", "Informative Coverage at Reference Cpgs", "Informative Coverage", "# CpGs", &coverage.ref_cpg_inf ),
		CovType::NonRefCpg =>("non_ref_cpg", "Coverage at Non Reference CpGs", "Coverage", "# CpGs", &coverage.non_ref_cpg ),
		CovType::NonRefCpgInf => ("non_ref_cpg_inf", "Informative Coverage at Non Reference Cpgs", "Informative Coverage", "# CpGs", &coverage.non_ref_cpg_inf ),
		CovType::Variant => ("variants", "Coverage at Variant Sites", "Coverage", "# sites", &coverage.variant ),		
	};
	let path: PathBuf = [dir, Path::new(format!("{}_coverage_{}.png", bc, name).as_str())].iter().collect();
	make_hist(&path, &ch, title, xaxis, yaxis).map_err(|e| format!("{}", e))?;
	Ok(())	
}

fn make_quality_graph(bc: &str, dir: &Path, qual: QualType, call_json: &CallJson) -> Result<(), String> {
	let quality = call_json.quality();
	let (name, title, yaxis, qv) = match qual {
		QualType::All => ("all", "Quality at All Sites", "# sites", &quality.all ),
		QualType::RefCpg =>("ref_cpg", "Quality at Reference CpGs", "# CpGs", &quality.ref_cpg ),
		QualType::NonRefCpg =>("non_ref_cpg", "Quality at Non Reference CpGs", "# CpGs", &quality.non_ref_cpg ),
		QualType::Variant => ("variants", "Quality at Variant Sites", "# sites", &quality.variant ),		
	};
	let path: PathBuf = [dir, Path::new(format!("{}_quality_{}.png", bc, name).as_str())].iter().collect();
	let mut th = HashMap::new();
	for(x,y) in qv.iter().enumerate() { th.insert(x, *y); } 
	if th.is_empty() { th.insert(0, 0); }	
	make_hist(&path, &th, title, "Quality", yaxis).map_err(|e| format!("{}", e))?;
	Ok(())	
}

fn make_qc_dist_graph(bc: &str, dir: &Path, qual: QCDistType, call_json: &CallJson) -> Result<(), String> {
	let qc_dist = call_json.qc_dist();
	let (name, title, xaxis, variant, qv) = match qual {
		QCDistType::QDVariant => ("qd_variant", "Allele specific variant call normalized by coverage for variant allele", "Quality by Depth", true, &qc_dist.quality_by_depth),
		QCDistType::QDNonVariant => ("qd_nonvariant", "Allele specific variant call normalized by coverage for non-variant allele", "Quality by Depth", false, &qc_dist.quality_by_depth),
		QCDistType::RMSVariant => ("rmsmq_variant", "RMS MapQ of reads support variant allele", "RMS MapQ", true, &qc_dist.rms_mapping_quality),
		QCDistType::RMSNonVariant => ("rmsmq_nonvariant", "RMS MapQ of reads support non-variant allele", "RMS MapQ", false, &qc_dist.rms_mapping_quality),
	};
	let path: PathBuf = [dir, Path::new(format!("{}_{}.png", bc, name).as_str())].iter().collect();
	let mut th = HashMap::new();
	if variant { for(x,y) in qv.iter() { th.insert(*x, y.variant()); }}
	else { for(x,y) in qv.iter() { th.insert(*x, y.non_variant()); }}	
	if th.is_empty() { th.insert(0, 0); }	
	make_hist(&path, &th, title, xaxis, "# sites").map_err(|e| format!("{}", e))?;
	Ok(())	
}

fn load_call_json(cjson: LoadCallJson) -> Result<(), String> {
	let rdr = compress::open_bufreader(&cjson.path).map_err(|e| format!("{}", e))?;
	let jstats = CallJson::from_reader(rdr)?;
	let mut cj = cjson.call_json.write().expect("Couldn't obtain write lock on CallJson structure");
	*cj = Some(jstats);
	debug!("Read in Call Json file {}", cjson.path.to_string_lossy());
	Ok(())	
}

fn make_heatmap(path: &Path, ch: &[(usize, &Vec<usize>)], title: &str, xlabel: &str, ylabel: &str, ylim: usize, max_z: usize) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(&path, (640, 480)).into_drawing_area();
	root.fill(&WHITE)?;
	
	let (area1, area2) = root.split_horizontally(512);
    let mut chart = ChartBuilder::on(&area1)
        .x_label_area_size(35)
        .y_label_area_size(60)
        .margin(5)
        .caption(title, ("sans-serif", 22.0).into_font())
        .build_ranged(-0.5..100.5, 0.5..ylim as f64 + 0.5)?;

    chart
        .configure_mesh()
        .line_style_1(&WHITE.mix(0.3))
        .y_desc(ylabel)
        .x_desc(xlabel)
        .disable_x_mesh()
        .disable_y_mesh()
		.y_label_formatter(&|x| format!("{:.0}", x))
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;

	fn rgb(z: f64) -> (u8, u8, u8) {
		let r = (z.sqrt() * 255.0) as u8;
		let g = (z * z * z * 255.0) as u8;
		let b =	(((z * 2.0 * std::f64::consts::PI).sin() * 0.5 + 0.5) * 255.0) as u8;
		(r, g, b)		
	}
	for (y,v) in ch {
		if *y > ylim { break; }
	    chart.draw_series(
			v.iter().enumerate().map(|(x, z)| {
				let x1 = x as f64 - 0.5;
				let y1 = *y as f64 - 0.5;
				let z1 = (*z as f64) / (max_z as f64);
				let (r, g, b) = rgb(z1);
				let cl = Into::<ShapeStyle>::into(&RGBColor(r, g, b)).filled();

				Rectangle::new([(x1, y1), (x1 + 1.0, y1 + 1.0)], cl)
			})
		)?;	 
	} 
    let mut chart = ChartBuilder::on(&area2)
		.right_y_label_area_size(50)
        .margin_bottom(40).margin_right(5).margin_left(30).margin_top(35)
        .build_ranged(0..1, 0..max_z)?;

    chart
        .configure_mesh()
        .line_style_1(&WHITE.mix(0.3))
        .y_desc("# sites")
        .disable_x_mesh().disable_y_mesh().disable_x_axis()
		.y_label_formatter(&|x| format!("{:.1e}", x))
      	.y_label_style(("sans-serif", 10).into_font())
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;
	
	let step = (max_z + 479) / 480;
	chart.draw_series(
		(0..max_z).step_by(step).map(|z| {
			let z1 = (z as f64) / (max_z as f64);
			let (r, g, b) = rgb(z1);
			let cl = Into::<ShapeStyle>::into(&RGBColor(r, g, b)).filled();
			Rectangle::new([(0, z), (1, z + step)], cl)
		})
			
	)?;
	Ok(())
}

fn make_gc_coverage_heatmap(bc: &str, dir: &Path, call_json: &CallJson) -> Result<(), String> {
	let path: PathBuf = [dir, Path::new(format!("{}_gc_coverage.png", bc).as_str())].iter().collect();
	let rf = &call_json.coverage().gc;
	let mut total = 0;
	let mut tv = Vec::new();
	for (x, v) in rf.iter() {
		let t: usize = v.iter().sum();
		total += t;
		tv.push((*x, v));
	}
	tv.sort_by(|a,b| a.0.cmp(&b.0));
	let lim = 0.99 * (total as f64);
	total = 0;
	let mut max = 0;
	let mut lim_cov = None;
	for (x, v) in tv.iter() {
		let t: usize = v.iter().sum();
		let m = v.iter().max().unwrap();
		if *m > max { max = *m }
		total += t;
		if total as f64 >= lim {
			lim_cov = Some(*x);
			break;
		}
	}
	let lim_cov = lim_cov.expect("No content found in GC distribution");
	make_heatmap(&path, &tv, "GC vs Coverage", "GC %", "Coverage", lim_cov, max).map_err(|e| format!("{}", e))	
}

fn make_meth_level_plot(path: &Path, vrf: &[&Vec<f64>; 4], title: &str, xlabel: &str, ylabel: &str, ymax: f64, names: &[&str; 4]) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(&path, (640, 480)).into_drawing_area();
	root.fill(&WHITE)?;
	let colours = [&MAGENTA, &RED, &GREEN, &BLUE];
	
    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(60)
        .margin(5)
        .caption(title, ("sans-serif", 22.0).into_font())
        .build_ranged(0.0..100.0, 0.0..ymax)?;

    chart
        .configure_mesh()
        .line_style_1(&WHITE.mix(0.3))
        .y_desc(ylabel)
        .x_desc(xlabel)
//        .disable_x_mesh()
//        .disable_y_mesh()
		.y_label_formatter(&|x| format!("{:.2}", x))
		.x_label_formatter(&|x| format!("{:.0}", x))
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;
	
	for (ix, v) in vrf.iter().enumerate() {
		let col = colours[ix];
		let tot: f64 = v.iter().sum();
		chart.draw_series(LineSeries::new(v.iter().enumerate().map(|(x, y)| {
			(x as f64, *y / tot)
		}), Into::<ShapeStyle>::into(col).stroke_width(3)))?
		.label(names[ix])
    	.legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], col));
	}
	chart.configure_series_labels().border_style(&BLACK).draw()?;
	Ok(())
}

fn make_meth_level_chart(bc: &str, dir: &Path, call_json: &CallJson) -> Result<(), String> {
	let path: PathBuf = [dir, Path::new(format!("{}_methylation_levels.png", bc).as_str())].iter().collect();
	let rf = &call_json.methylation();
	let vrf = [&rf.all_ref_cpg, &rf.passed_ref_cpg, &rf.all_non_ref_cpg, &rf.passed_non_ref_cpg];
	let mut max = 0.0;
	for v in vrf.iter() {
		let z: f64 = v.iter().sum();
		let m: f64 = *v.iter().max_by(|a,b| a.partial_cmp(b).unwrap()).unwrap();
		if m / z > max { max = m / z}
	}
	let names = ["All Ref CpG", "Passed Ref CpG", "All Non-Ref CpG", "Passed Non-Ref CpG"];
	make_meth_level_plot(&path, &vrf, "Methylation Levels", "% Methylation","% CpGs", max, &names).map_err(|e| format!("{}", e))
}

fn make_noncpg_profile_plot(path: &Path, v: &[f64], title: &str, xlabel: &str, ylabel: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(&path, (640, 480)).into_drawing_area();
	root.fill(&WHITE)?;
	let max: f64 = *v.iter().max_by(|a,b| a.partial_cmp(b).unwrap()).unwrap();
	
    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(60)
        .margin(5)
        .caption(title, ("sans-serif", 22.0).into_font())
        .build_ranged(0..v.len(), 0.0..max)?;

    chart
        .configure_mesh()
        .line_style_1(&WHITE.mix(0.3))
        .y_desc(ylabel)
        .x_desc(xlabel)
		.y_label_formatter(&|x| format!("{:.2}", x))
		.x_label_formatter(&|x| format!("{:.0}", x))
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;
	
	chart.draw_series(LineSeries::new(v.iter().enumerate().map(|(x, y)| (x, *y)), Into::<ShapeStyle>::into(&RED).stroke_width(3)))?;
	Ok(())
}

fn make_noncpg_read_profile(bc: &str, dir: &Path, call_json: &CallJson) -> Result<(), String> {
	let path: PathBuf = [dir, Path::new(format!("{}_non_cpg_read_profile.png", bc).as_str())].iter().collect();
	let rf = &call_json.methylation().non_cpg_read_profile;
	let f = |a: &[usize]| -> f64 { if (a[0] + a[1]) > 0 { a[0] as f64 / (a[0] + a[1]) as f64 } else { 0.0 }};
	match rf {
		None => {
			std::fs::File::create(path).map_err(|e| format!("{}", e))?;
			Ok(())
		},	
		Some(v) => {
			let tv: Vec<f64> = v.iter().map(|x| 100.0 * f(x)).collect();
			make_noncpg_profile_plot(&path, &tv, "% Non-Conversion at Non-CpG Sites", "Position in Read", "% Non-Conversion").map_err(|e| format!("{}", e))		
		},
	}
}

fn make_read_level_table(json: &CallJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("hor-zebra");
	table.add_header(vec!("Type", "# Reads", "%", "# Bases", "%"));
	let fs = json.filter_stats();
	let rl = fs.read_level();
	let tot = fs.read_level_totals();
	let f = |name: &str, x: FSCounts| {
		let mut row = vec!(name.to_owned());
		row.push(format!("{}", x.reads()));
		row.push(format!("{:.2}", pct(x.reads(), tot.reads())));
		row.push(format!("{}", x.bases()));
		row.push(format!("{:.2}", pct(x.bases(), tot.bases())));		
		row
	};
	table.add_row(f("Total", tot));
	for(key, name) in FSReadLevelType::iter() {
		if let Some(t) = rl.get(&key) { table.add_row(f(name, *t)); }
	}
	Ok(Content::Table(table))
}

fn make_base_level_table(json: &CallJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("green");
	table.add_header(vec!("Bases", "#", "%"));
	let fs = json.filter_stats();
	let rl = fs.base_level();
	let tot = fs.base_level_totals();
	let f = |name: &str, x| {
		let mut row = vec!(name.to_owned());
		row.push(format!("{}", x));
		row.push(format!("{:.2}", pct(x, tot)));
		row	
	};
	table.add_row(f("Total", tot));
	for(key, name) in FSBaseLevelType::iter() {
		if let Some(t) = rl.get(&key) { table.add_row(f(name, *t)); }
	}
	Ok(Content::Table(table))
}

fn make_variant_count_table(json: &CallJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("green");
	table.add_header(vec!("Type", "Total", "Passed", "% Passed"));
	let bs = json.basic_stats();
	let f = |s: &str, ct: &Counts, tab: &mut HtmlTable, opt: bool| {
		if !opt || ct.all() > 0 {
			tab.add_row(vec!(s.to_owned(), format!("{}", ct.all()), format!("{}", ct.passed()), format!("{:.2}", pct(ct.passed(), ct.all()))));
		} 
	};
	f("SNPs", bs.snps(), &mut table, false);
	f("Indels", bs.indels(), &mut table, true);
	f("Multi-allelic", bs.multiallelic(), &mut table, true);	
	Ok(Content::Table(table))	
}

fn make_vcf_filter_stats_table(json: &CallJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("hor-zebra");
	table.add_header(vec!("Type", "# Sites", "%", "# Non-Variant Sites", "%", "# Variant Sites", "%"));
	let mut v = Vec::new();
	let mut pass = None;
	let mut all = QCCounts::new();
	for(k, ct) in json.vcf_filter_stats().iter() {
		if k == "PASS" { pass = Some(ct); }
		else { v.push((k, *ct)); }
		all += *ct;
	}
	let pass = if let Some(x) = pass { *x } else { return Err("No passed variant information found".to_string()); };
	v.sort_by(|(_, a), (_, b)| (*b).all().cmp(&(*a).all()));
	let f = |s: &str, ct: QCCounts, tot: usize| vec!(
		s.to_owned(),
		format!("{}",ct.all()), format!("{:.2}", pct(ct.all(), tot)),
		format!("{}",ct.non_variant()), format!("{:.2}", pct(ct.non_variant(), tot)),
		format!("{}",ct.variant()), format!("{:.2}", pct(ct.variant(), tot))
	);
	table.add_row(f("All", all, all.all()));
	table.add_row(Vec::new());
	table.add_row(f("Passed", pass, all.all()));
	let flt = all - pass;
	table.add_row(f("Filtered", flt, all.all()));
	table.add_row(Vec::new());
	for(k, ct) in v.iter() { if ct.all() > 0 { table.add_row(f(k, *ct, flt.all())); }}
	Ok(Content::Table(table))		
}

fn new_body(project: &str, bc: &str, tag: &str) -> HtmlElement {
	let mut body = HtmlElement::new("BODY", None, true);
	let mut path = HtmlElement::new("P", Some("id=\"path\""), true);
	path.push_string(format!("/{}/{}/{}", project, bc, tag)); 
	body.push(Content::Element(path));
	let mut back = HtmlElement::new("B", None, true);
	back.push_str("BACK");
	let mut back_link = HtmlElement::new("a", Some("class=\"link\" href=\"../index.html\""), true);
	back_link.push_element(back);
	body.push_element(back_link);
	body.push_element(make_title(format!("SAMPLE {}", bc)));
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body
}

fn create_mapping_report_body(project: &str, bc: &str, dir: &Path, json: &CallJson) -> Result<HtmlElement, String> {
	let mut img_dir = dir.to_owned();
	img_dir.push("images");
	let mut body = new_body(project, bc, "mapping_coverage");
	body.push_element(make_section("Read Level Counts"));
	body.push(make_read_level_table(json)?);
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("Base Level Counts"));
	body.push(make_base_level_table(json)?);
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("Coverage and Quality"));
	let get_path = |name: &str| {
		let mut tp = img_dir.clone();
		tp.push(format!("{}_{}.png", bc, name).as_str());
		tp
	};
	let img_str = |p: &Path| {
		let fname = p.file_name().expect("Missing filename").to_string_lossy();
		format!("<img src=\"images/{}\" alt=\"{}\">", fname, fname)	
	};
	let mut table = HtmlTable::new("hor-zebra");
	table.add_header(vec!("Coverage Distribution", "Quality Distribution"));
	table.add_row(vec!(img_str(&get_path("coverage_all")), img_str(&get_path("quality_all"))));
	body.push(Content::Table(table));
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	table = HtmlTable::new("green");
	table.add_header(vec!("GC/Coverage heatmap", "% Non-Conversion at Non-CpG Sites"));
	table.add_row(vec!(img_str(&get_path("gc_coverage")), img_str(&get_path("non_cpg_read_profile"))));
	body.push(Content::Table(table));	
	Ok(body)
}

fn create_variant_report_body(project: &str, bc: &str, dir: &Path, json: &CallJson) -> Result<HtmlElement, String> {
	let mut img_dir = dir.to_owned();
	img_dir.push("images");
	let mut body = new_body(project, bc, "variants");
	body.push_element(make_section("Variant Counts"));
	body.push(make_variant_count_table(json)?);
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("VCF Filter Stats"));
	body.push(make_vcf_filter_stats_table(json)?);
	Ok(body)
}

fn new_page(path: &Path) -> Result<HtmlPage, String> {
	let mut html = HtmlPage::new(path)?;
	let mut head_element = HtmlElement::new("HEAD", None, true);
	let mut style_element = HtmlElement::new("STYLE", Some("TYPE=\"text/css\""), true);
	style_element.push_str("<!--\n@import url(\"../../css/style.css\");\n-->");
	head_element.push_element(style_element);
	html.push_element(head_element);
	Ok(html)
}

fn create_mapping_report(bc: &str, dir: &Path, project: &str, call_json: &CallJson) -> Result<(), String> {
	let path: PathBuf = [dir, Path::new(format!("{}_mapping_coverage.html", bc).as_str())].iter().collect();
	let mut html = new_page(&path)?;
	html.push_element(create_mapping_report_body(project, bc, dir, call_json)?);	
	Ok(())
}

fn create_variant_report(bc: &str, dir: &Path, project: &str, call_json: &CallJson) -> Result<(), String> {
	let path: PathBuf = [dir, Path::new(format!("{}_variants.html", bc).as_str())].iter().collect();
	let mut html = new_page(&path)?;
	html.push_element(create_variant_report_body(project, bc, dir, call_json)?);	
	Ok(())
}

fn make_call_job(bc: &str, bc_dir: &Path, project: &str, job: MakeCallJob) -> Result<(), String> {
	let t = job.call_json.read().expect("Couldn't obtain read lock on CallJson structure");
	let cj = t.as_ref().expect("No CallJson struct found");
	let mut img_dir = bc_dir.to_owned();
	img_dir.push("images");
	match job.job_type {
		CallJob::CoverageAll => make_coverage_graph(bc, &img_dir, CovType::All, cj),
		CallJob::CoverageRefCpg => make_coverage_graph(bc, &img_dir, CovType::RefCpg, cj),
		CallJob::CoverageRefCpgInf => make_coverage_graph(bc, &img_dir, CovType::RefCpgInf, cj),
		CallJob::CoverageNonRefCpg => make_coverage_graph(bc, &img_dir, CovType::NonRefCpg, cj),
		CallJob::CoverageNonRefCpgInf => make_coverage_graph(bc, &img_dir, CovType::NonRefCpgInf, cj),
		CallJob::CoverageVariant => make_coverage_graph(bc, &img_dir, CovType::Variant, cj),
		CallJob::QualityAll => make_quality_graph(bc, &img_dir, QualType::All, cj),
		CallJob::QualityRefCpg => make_quality_graph(bc, &img_dir, QualType::RefCpg, cj),
		CallJob::QualityNonRefCpg => make_quality_graph(bc, &img_dir, QualType::NonRefCpg, cj),
		CallJob::QualityVariant => make_quality_graph(bc, &img_dir, QualType::Variant, cj),
		CallJob::QdVariant => make_qc_dist_graph(bc, &img_dir, QCDistType::QDVariant, cj),
		CallJob::QdNonVariant => make_qc_dist_graph(bc, &img_dir, QCDistType::QDNonVariant, cj),
		CallJob::RmsMqVariant => make_qc_dist_graph(bc, &img_dir, QCDistType::RMSVariant, cj),
		CallJob::RmsMqNonVariant => make_qc_dist_graph(bc, &img_dir, QCDistType::RMSNonVariant, cj),
		CallJob::GCCoverage => make_gc_coverage_heatmap(bc, &img_dir, cj),
		CallJob::MethylationLevels => make_meth_level_chart(bc, &img_dir, cj),
		CallJob::NonCpgReadProfile => make_noncpg_read_profile(bc, &img_dir, cj),
		CallJob::FsVariants => {
			let path: PathBuf = [&img_dir, Path::new(format!("{}_fs_variant.png", bc).as_str())].iter().collect();
			make_hist(&path, &cj.qc_dist().fisher_strand, "Fisher Strand Test", "Fisher Strand Phred Scale Probability", "# sites").map_err(|e| format!("{}", e))
		},
		CallJob::MappingReport => create_mapping_report(bc, bc_dir, project, cj),
		CallJob::VariantReport => create_variant_report(bc, bc_dir, project, cj),
		_ => Ok(()),
	}
}

fn handle_call_job(job: ReportJob) -> Result<(), String> {
	match job.job {
		RepJob::CallJson(v) => {
			load_call_json(v)
		},
		RepJob::CallJob(v) => {
			make_call_job(&job.barcode, &job.bc_dir, &job.project, v)
		},
		_ => Err("Invalid command".to_string())
	}
}

fn worker_thread(tx: mpsc::Sender<(isize, usize)>, rx: mpsc::Receiver<Option<ReportJob>>, idx: isize) -> Result<(), String> {
	loop {
		match rx.recv() {
			Ok(Some(job)) => {
				let job_ix = job.ix;
				if let Err(e) = handle_call_job(job) {
					error!("Error handling call report job: {}", e);
					tx.send((-(idx + 1), job_ix)).expect("Error sending message to parent");
				} else {
					tx.send((idx, job_ix)).expect("Error sending message to parent");
				}
			},
			Ok(None) => {
				debug!("Call report thread {} received signal to shutdown", idx);
				break;
			}
			Err(e) => {
				error!("Call report thread {} received error: {}", idx, e);
				break;
			}
		}
	}
	debug!("Call report thread {} shutting down", idx);
	Ok(())
}

fn prepare_jobs(svec: &[CallJsonFiles], project: &str) -> Vec<ReportJob> {
	let mut v = Vec::new();
	for cjson in svec.iter() {
		let call_json = Arc::new(RwLock::new(None));
		let load_json = LoadCallJson{path: cjson.json_file.clone(), call_json: call_json.clone()};
		let ld_json_ix = v.len();
		v.push(ReportJob::new(&cjson.barcode, project, &cjson.bc_dir, RepJob::CallJson(load_json)));
		for job_type in CallJob::iter() {
			let mk_graph = MakeCallJob{job_type, depend: ld_json_ix, call_json: call_json.clone()};
			v.push(ReportJob::new(&cjson.barcode, project, &cjson.bc_dir, RepJob::CallJob(mk_graph)));
		}
	}
	for (ix, job) in v.iter_mut().enumerate() { job.ix = ix }
	v
}

pub fn make_call_report(sig: Arc<AtomicUsize>, outputs: &[PathBuf], project: Option<String>, css: &Path, n_cores: usize, svec: Vec<CallJsonFiles>) -> Result<Option<Box<dyn BufRead>>, String> {
	utils::check_signal(Arc::clone(&sig))?;
	let project = project.unwrap_or_else(|| "gemBS".to_string());
	let output_dir = outputs.first().expect("No output files for map report").parent().expect("No parent directory found for map report");
	// Set up worker threads	
	// Maximum parallel jobs that we could do if there were enough cores is 18 * the number of samples (18 images per sample)
	let n_dsets = svec.len() * 18;
	let n_workers = if n_cores > n_dsets { n_dsets } else { n_cores };
	let mut job_vec = prepare_jobs(&svec, &project);
	let (ctr_tx, ctr_rx) = mpsc::channel();
	let mut avail = Vec::new();
	let mut workers = Vec::new();
	let mut jobs = Vec::new();
	for ix in 0..n_workers {
		let (tx, rx) = mpsc::channel();
		let ctr = mpsc::Sender::clone(&ctr_tx);
		let handle = thread::spawn(move || { worker_thread(ctr, rx, ix as isize)});
		workers.push(Worker{handle, tx, ix});
		avail.push(ix);
	}
	let mut abort = false;
	loop {
		utils::check_signal(Arc::clone(&sig))?;
		let worker_ix = avail.pop();
		let (job_ix, waiting) = if worker_ix.is_some() {
			let mut x = None;
			let mut waiting = false;
			for (ix, rjob) in job_vec.iter().enumerate() {
				if rjob.status == JobStatus::Ready {
					match &rjob.job {
						RepJob::CallJson(_) => {
							x = Some(ix);
							waiting = false;
							break;
						},
						RepJob::CallJob(v) => {
							if job_vec[v.depend].status == JobStatus::Completed {
								x = Some(ix);
								waiting = false;
								break;
							} else { waiting = true; }
							
						},
						_ => (),
					}
				}
			}
			(x, waiting)
		} else {
			(None, true)
		};
		if let Some(jix) = job_ix { 
			job_vec[jix].status = JobStatus::Running;
			let idx = worker_ix.expect("No worker index");
			jobs.push(idx as isize);
			debug!("Sending call report job to worker {}", idx);			
			workers[idx].tx.send(Some(job_vec[jix].clone())).expect("Error sending new command to map report worker thread");
			match ctr_rx.try_recv() {
				Ok((x, ix)) if x >= 0 => {
					debug!("Job completion by call worker thread {}", x);
					jobs.retain(|ix| *ix != x);
					avail.push(x as usize);
					job_vec[ix].status = JobStatus::Completed;
				},
				Ok((x, _)) => {
					error!("Error received from worker thread {}", -(x+1));
					abort = true;
					break;
				},
				Err(mpsc::TryRecvError::Empty) => {},
				Err(e) => {
					error!("Scheduler thread received error: {}", e);
					abort = true;
					break;
				}				
			}							
		} else { 
			if let Some(idx) = worker_ix { avail.push(idx); } 			
			if !jobs.is_empty() {
				match ctr_rx.recv_timeout(time::Duration::from_millis(1000)) {
					Ok((x, ix)) if x >= 0 => {
						debug!("Job completion by worker thread {}", x);
						jobs.retain(|ix| *ix != x);
						avail.push(x as usize);
						job_vec[ix].status = JobStatus::Completed;
					},
					Ok((x, _)) => {
						error!("Error received from worker thread {}", -(x+1));
						abort = true;
						break;
					},
					Err(mpsc::RecvTimeoutError::Timeout) => {},
					Err(e) => {
						error!("Scheduler thread received error: {}", e);
						abort = true;
						break;
					}				
				}
			} else if waiting { thread::sleep(time::Duration::from_secs(1)) }
			else { break; }
		}
	}
	if !abort {
		for w in workers.drain(..) {
			if w.tx.send(None).is_err() {
				debug!("Error when trying to send shutdown signal to worker thread {}", w.ix);
				abort = true;
				break;
			}
			if w.handle.join().is_err() { 
				debug!("Error received from worker {} at join", w.ix);
				abort = true;
				break;
			}
		}
	}
	if abort { Err("Map-report generation failed".to_string()) }
	else {
		make_map_report::copy_css(output_dir, css)?;
		Ok(None) 

	}
}

use std::path::{Path, PathBuf};
use std::io::BufRead;
use std::str::FromStr;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, mpsc, Mutex, RwLock};
use std::{thread, time};
use std::collections::{HashMap, HashSet};

use plotters::prelude::*;

use crate::common::{utils, compress};
use crate::common::json_call_stats::{CallJson, FSReadLevelType, FSBaseLevelType, FSCounts, Counts, QCCounts, MutCounts};
use crate::scheduler::report::CallJsonFiles;
use super::report_utils::*;
use super::make_map_report;
use super::make_map_report::{make_title, make_section};
use crate::common::html_utils::*;
use crate::common::latex_utils::*;

enum CovType { All, NonRefCpg, NonRefCpgInf, RefCpg, RefCpgInf, Variant }
enum QualType { All, RefCpg, NonRefCpg, Variant }
enum QCDistType { QDVariant, QDNonVariant, RMSVariant, RMSNonVariant }

fn prep_hist_vec(ch: &HashMap<usize, usize>) -> (Vec<(usize, usize)>, usize, usize) {
	let mut total = 0;
	let mut m = 0;
	let mut t = Vec::new();
	for(x,y) in ch.iter() {
		total += y;
		if *y > m { m = *y; }
		t.push((*x,*y));
	}
	t.sort_by(|a,b| a.0.cmp(&b.0));
	(t, total, m)	
}
fn make_hist(path: &Path, ch: &HashMap<usize, usize>, title: &str, xlabel: &str, ylabel: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(&path, (640, 480)).into_drawing_area();
	root.fill(&WHITE)?;
	
	let(t, total, m) = prep_hist_vec(ch);
	let lim_y = 0.99 * (total as f64);
	let mut lim_x = t[0].0;
	let mut tot = 0;
	for(x, y) in t.iter() {
		tot += y;
		if (tot as f64) >= lim_y { break; }
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
		QCDistType::QDVariant => ("qd_variant", "Quality by depth for variant allele", "Quality by Depth", true, &qc_dist.quality_by_depth),
		QCDistType::QDNonVariant => ("qd_nonvariant", "Quality by depth for non-variant allele", "Quality by Depth", false, &qc_dist.quality_by_depth),
		QCDistType::RMSVariant => ("rmsmq_variant", "RMS MapQ of variant allele reads", "RMS MapQ", true, &qc_dist.rms_mapping_quality),
		QCDistType::RMSNonVariant => ("rmsmq_nonvariant", "RMS MapQ of non-variant allele reads", "RMS MapQ", false, &qc_dist.rms_mapping_quality),
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

fn prep_gc_vec(rf: &HashMap<usize, Vec<usize>>) -> (Vec<(usize, &Vec<usize>)>, Option<usize>) {
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
	let mut lim_x = None;
	for (x, v) in tv.iter() {
		let t: usize = v.iter().sum();
		total += t;
		if total as f64 >= lim {
			lim_x = Some(*x);
			break;
		}
	}	
	(tv, lim_x)
}

fn make_gc_coverage_heatmap(bc: &str, dir: &Path, call_json: &CallJson) -> Result<(), String> {
	let path: PathBuf = [dir, Path::new(format!("{}_gc_coverage.png", bc).as_str())].iter().collect();
	let rf = &call_json.coverage().gc;
	let(tv, lim_cov) = prep_gc_vec(rf);
	let lim_cov = lim_cov.expect("No content found in GC distribution");
	let mut max = 0;
	for (x, v) in tv.iter() {
		if *x > lim_cov { break; }
		let m = v.iter().max().unwrap();
		if *m > max { max = *m }
	}
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

fn calc_gc_corr(json: &CallJson) -> f64 {
	// Calc gc/depth_correlation
	let(tv, lim_cov) = prep_gc_vec(&json.coverage().gc);
	let lim_cov = lim_cov.expect("No content in gc histogram");
	let (mut n, mut sx, mut sy, mut sx2, mut sy2, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
	for (x, v) in tv.iter() {
		if *x > lim_cov { break; }
		let cov = *x as f64;
		for (ix, y) in v.iter().enumerate() {
			let gc = (ix as f64) * 0.01;
			let z = *y as f64;
			n += z;
			sx += z * gc;
			sx2 += z * gc * gc;
			sy += z * cov;
			sy2 += z * cov * cov;
			sxy += z * gc * cov;		
		}
	}
	let tz = (n * sx2 - sx * sx) * (n * sy2 - sy * sy);
	if tz > 0.0 { (n * sxy - sx * sy) / tz.sqrt() } else { 0.0 }
}

fn make_read_level_tab<T: Table>(table: &mut T, json: &CallJson, ms: Option<&mut MapSummary>) -> Result<(), String> {
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
	if let Some(msumm) = ms {
		msumm.aligned = tot.bases();
		msumm.unique = tot.bases() - rl.get(&FSReadLevelType::LowMAPQ).map(|x| x.bases()).unwrap_or(0);
		msumm.passed =  rl.get(&FSReadLevelType::Passed).map(|x| x.bases()).unwrap_or(0);
		msumm.gc_correlation = calc_gc_corr(json);
	}
	Ok(())
}

fn make_read_level_table(json: &CallJson, msumm: &mut MapSummary) -> Result<Content, String> {
	let mut table = HtmlTable::new("hor-zebra");
	make_read_level_tab(&mut table, json, Some(msumm))?;
	Ok(Content::Table(table))
}

fn make_read_level_latex_tab(json: &CallJson) -> Result<LatexContent, String> {
	let mut table = LatexTable::new();
	make_read_level_tab(&mut table, json, None)?;
	Ok(LatexContent::Table(table))
}

fn make_base_level_tab<T: Table>(table: &mut T, json: &CallJson) -> Result<(), String> {
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
	Ok(())
}

fn make_base_level_table(json: &CallJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("green");
	make_base_level_tab(&mut table, json)?;
	Ok(Content::Table(table))
}

fn make_base_level_latex_tab(json: &CallJson) -> Result<LatexContent, String> {
	let mut table = LatexTable::new();
	make_base_level_tab(&mut table, json)?;
	Ok(LatexContent::Table(table))
}

fn make_variant_count_tab<T: Table>(table: &mut T, json: &CallJson) -> Result<(), String> {
	table.add_header(vec!("Type", "Total", "Passed", "% Passed"));
	let bs = json.basic_stats();
	let f = |s: &str, ct: &Counts, tab: &mut T, opt: bool| {
		if !opt || ct.all() > 0 {
			tab.add_row(vec!(s.to_owned(), format!("{}", ct.all()), format!("{}", ct.passed()), format!("{:.2}", pct(ct.passed(), ct.all()))));
		} 
	};
	f("SNPs", bs.snps(), table, false);
	f("Indels", bs.indels(), table, true);
	f("Multi-allelic", bs.multiallelic(), table, true);	
	Ok(())
}

fn make_variant_count_table(json: &CallJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("hor-zebra");
	make_variant_count_tab(&mut table, json)?;
	Ok(Content::Table(table))
}

fn make_variant_count_latex_tab(json: &CallJson) -> Result<LatexContent, String> {
	let mut table = LatexTable::new();
	make_variant_count_tab(&mut table, json)?;
	Ok(LatexContent::Table(table))
}

fn make_cpg_count_tab<T: Table>(table: &mut T, json: &CallJson, ms: Option<&mut MethSummary>) -> Result<(), String> {
	table.add_header(vec!("Type", "Total", "Passed", "% Passed"));
	let bs = json.basic_stats();
	let f = |s: &str, ct: &Counts, tab: &mut T, opt: bool| {
		if !opt || ct.all() > 0 {
			tab.add_row(vec!(s.to_owned(), format!("{}", ct.all()), format!("{}", ct.passed()), format!("{:.2}", pct(ct.passed(), ct.all()))));
		} 
	};
	f("Reference Cpgs", bs.ref_cpg(), table, false);
	f("Non Reference Cpgs", bs.non_ref_cpg(), table, true);
	if let Some(msumm) = ms { msumm.passed_cpgs = bs.ref_cpg().passed() + bs.non_ref_cpg().passed(); }
	Ok(())	
}

fn make_cpg_count_table(json: &CallJson, msumm: &mut MethSummary) -> Result<Content, String> {
	let mut table = HtmlTable::new("hor-zebra");
	make_cpg_count_tab(&mut table, json, Some(msumm))?;
	Ok(Content::Table(table))
}

fn make_cpg_count_latex_tab(json: &CallJson) -> Result<LatexContent, String> {
	let mut table = LatexTable::new();
	make_read_level_tab(&mut table, json, None)?;
	Ok(LatexContent::Table(table))
}

fn make_cpg_meth_profile_tab<T: Table>(table: &mut T, json: &CallJson, ms: Option<&mut MethSummary>) -> Result<(), String> {
	table.add_header(vec!("Type", "All Ref. CpGs (%)", "Passed (%)", "All Non Ref. CpGs (%)", "Passed (%)"));
	let rf = &json.methylation();
	let vrf = [&rf.all_ref_cpg, &rf.passed_ref_cpg, &rf.all_non_ref_cpg, &rf.passed_non_ref_cpg];
	let mut stats = Vec::new();
	for v in vrf.iter() {
		let z: f64 = v.iter().sum();
		let mut y = 0.0;
		let mut median = None;
		let mut sums = [0.0, 0.0, 0.0];
		for (ix, x) in v.iter().enumerate() {
			y += x;
			if median.is_none() && y / z >= 0.5 { median = Some((ix as f64) * 0.01); }
			if ix < 30 { sums[0] += *x; }
			else if ix < 70 { sums[1] += *x; }
			else { sums[2] += *x; }
		}
		stats.push((median.expect("No median found!"), z, sums[0], sums[1], sums[2]));
	}
	let f = |a, b| { if b > 0.0 { 100.0 * a / b } else { 0.0 } };
	table.add_row(vec!(
		"m < 0.3".to_string(), format!("{:.0} ({:.2})",stats[0].2, f(stats[0].2, stats[0].1)),
		format!("{:.0} ({:.2}%)",stats[1].2, f(stats[1].2, stats[1].1)), 
		format!("{:.0} ({:.2}%)",stats[2].2, f(stats[2].2, stats[2].1)), 
		format!("{:.0} ({:.2}%)",stats[3].2, f(stats[3].2, stats[3].1))));
	table.add_row(vec!(
		"0.3 <= m < 0.7".to_string(), format!("{:.0} ({:.2})",stats[0].3, f(stats[0].3, stats[0].1)),
		format!("{:.0} ({:.2}%)",stats[1].3, f(stats[1].3, stats[1].1)), 
		format!("{:.0} ({:.2}%)",stats[2].3, f(stats[2].3, stats[2].1)), 
		format!("{:.0} ({:.2}%)",stats[3].3, f(stats[3].3, stats[3].1))));
	table.add_row(vec!(
		"m >= 0.7".to_string(), format!("{:.0} ({:.2})",stats[0].4, f(stats[0].4, stats[0].1)),
		format!("{:.0} ({:.2}%)",stats[1].4, f(stats[1].4, stats[1].1)), 
		format!("{:.0} ({:.2}%)",stats[2].4, f(stats[2].4, stats[2].1)), 
		format!("{:.0} ({:.2}%)",stats[3].4, f(stats[3].4, stats[3].1))));
	table.add_row(Vec::new());
	table.add_row(vec!(
		"Median".to_string(), format!("{:0}", stats[0].0), format!("{:0}", stats[1].0),
		format!("{:0}", stats[2].0), format!("{:0}", stats[3].0)));
	if let Some(msumm) = ms { msumm.med_cpg_meth = stats[0].0; }
	Ok(())	
}

fn make_cpg_meth_profile_table(json: &CallJson, msumm: &mut MethSummary) -> Result<Content, String> {
	let mut table = HtmlTable::new("hor-zebra");
	make_cpg_meth_profile_tab(&mut table, json, Some(msumm))?;
	Ok(Content::Table(table))
}

fn make_cpg_meth_profile_latex_tab(json: &CallJson) -> Result<LatexContent, String> {
	let mut table = LatexTable::new();
	table.set_col_desc("|m{3cm}|m{3cm}|m{3cm}|m{3cm}|m{3cm}|");
	make_cpg_meth_profile_tab(&mut table, json, None)?;
	Ok(LatexContent::Table(table))
}

fn make_variant_type_tab<T: Table>(table: &mut T, table1: &mut T, json: &CallJson, vs: Option<&mut VarSummary>) -> Result<(), String> {
	let mutations = json.mutations();
	let mut ct_ti = MutCounts::new();
	let mut ct_tv = MutCounts::new();
	let mut transitions = HashSet::new();
	for k in &["A>G", "G>A", "T>C", "C>T"] { transitions.insert(k); }
	let mut transversions = HashSet::new();
	for k in &["A>C", "A>T", "C>A", "C>G", "G>C", "G>T", "T>A", "T>G"] { transversions.insert(k); }
	let mut v_ti = Vec::new();
	let mut v_tv = Vec::new();
	for(k, c) in mutations.iter() {
		if transitions.contains(&(k.as_str())) { 
			v_ti.push((k, *c));
			ct_ti += *c; 
		} else if transversions.contains(&(k.as_str())) { 
			v_tv.push((k, *c));
			ct_tv += *c;
		} else { return Err(format!("Unknown variant type {} from CallJson file", k)); }
	}
	let ct = ct_ti + ct_tv;
	let dbsnp = ct.dbsnp_all() > 0;
	let mut hdr = vec!("Mutation", "Type", "# All", "%", "# Passed", "%");
	if dbsnp {	hdr.extend(&["dbSNP All", "%", "dbSNP Passed", "%"]); } 
	table.add_header(hdr);
	v_ti.sort_by(|(a, _), (b, _)| a.cmp(b));
	v_tv.sort_by(|(a, _), (b, _)| a.cmp(b));
	let f = |k: &String, c: &MutCounts, s: &str| {
		let mut row = vec!(
			k.to_owned(), s.to_string(), 
			format!("{}", c.all()), format!("{:.2}", pct(c.all(), ct.all())),
			format!("{}", c.passed()), format!("{:.2}", pct(c.passed(), ct.passed())),
		);
		if dbsnp {	row.extend(vec!(
			format!("{}", c.dbsnp_all()), format!("{:.2}", pct(c.dbsnp_all(), ct.dbsnp_all())),
			format!("{}", c.dbsnp_passed()), format!("{:.2}", pct(c.dbsnp_passed(), ct.dbsnp_passed())),				
		));} 
		row
	};
	for (k, c) in v_ti.iter() { table.add_row(f(*k, c, "Transition")); }
	table.add_row(Vec::new());
	for (k, c) in v_tv.iter() { table.add_row(f(*k, c, "Transversion")); }
	table1.add_header(vec!("Status", "Ratio", "Transitions", "Transversions"));
	let ratio = |a, b| if b > 0 { (a as f64) / (b as f64) } else { 0.0 };
	let g = |a, b, s: &str| vec!(s.to_owned(), format!("{:.2}", ratio(a, b)), format!("{}", a), format!("{}", b));
	table1.add_row(g(ct_ti.all(), ct_tv.all(), "All"));
	table1.add_row(g(ct_ti.passed(), ct_tv.passed(), "Passed"));
	if let Some(vsumm) = vs { vsumm.ti_tv_ratio = ratio(ct_ti.passed(), ct_tv.passed()); }
	if dbsnp {
		table1.add_row(g(ct_ti.dbsnp_all(), ct_tv.dbsnp_all(), "dbSNP All"));
		table1.add_row(g(ct_ti.dbsnp_passed(), ct_tv.dbsnp_passed(), "dbSNP Passed"));
	}
	Ok(())
}

fn make_variant_type_tables(json: &CallJson, vsumm: &mut VarSummary) -> Result<(Content, Content), String> {
	let mut table = HtmlTable::new("green");
	let mut table1 = HtmlTable::new("hor-zebra");
	make_variant_type_tab(&mut table, &mut table1, json, Some(vsumm))?;
	Ok((Content::Table(table), Content::Table(table1)))		
}

fn make_variant_type_latex_tabs(json: &CallJson) -> Result<(LatexContent, LatexContent), String> {
	let mut table = LatexTable::new();
	let mut table1 = LatexTable::new();
	make_variant_type_tab(&mut table, &mut table1, json, None)?;
	Ok((LatexContent::Table(table), LatexContent::Table(table1)))		
}


fn make_vcf_filter_stats_tab<T: Table>(table: &mut T, json: &CallJson, vs: Option<&mut VarSummary>) -> Result<(), String> {
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
	if let Some(vsumm) = vs {
		vsumm.variants = all.all();
		vsumm.variants_passed = pass.all();
	}
	Ok(())
}

fn make_vcf_filter_stats_table(json: &CallJson, vsumm: &mut VarSummary) -> Result<Content, String> {
	let mut table = HtmlTable::new("green");
	make_vcf_filter_stats_tab(&mut table, json, Some(vsumm))?;
	Ok(Content::Table(table))		
}

fn make_vcf_filter_stats_latex_tab(json: &CallJson) -> Result<LatexContent, String> {
	let mut table = LatexTable::new();
	make_vcf_filter_stats_tab(&mut table, json, None)?;
	Ok(LatexContent::Table(table))		
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

fn create_mapping_report_body(project: &str, bc: &str, dir: &Path, json: &CallJson, msumm: &mut MapSummary) -> Result<HtmlElement, String> {
	let mut img_dir = dir.to_owned();
	img_dir.push("images");
	let mut body = new_body(project, bc, "mapping_coverage");
	body.push_element(make_section("Read Level Counts"));
	body.push(make_read_level_table(json, msumm)?);
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
	table.add_header(vec!("GC/Coverage Heatmap", "% Non-Conversion at Non-CpG Sites"));
	table.add_row(vec!(img_str(&get_path("gc_coverage")), img_str(&get_path("non_cpg_read_profile"))));
	body.push(Content::Table(table));	
	Ok(body)
}

fn create_variant_report_body(project: &str, bc: &str, dir: &Path, json: &CallJson, vsumm: &mut VarSummary) -> Result<HtmlElement, String> {
	let mut img_dir = dir.to_owned();
	img_dir.push("images");
	let mut body = new_body(project, bc, "variants");
	body.push_element(make_section("Variant Counts"));
	body.push(make_variant_count_table(json)?);
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("VCF Filter Stats"));
	body.push(make_vcf_filter_stats_table(json, vsumm)?);
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
	let mut table = HtmlTable::new("green");
	table.add_header(vec!("Coverage Distribution for Variant Sites", "Quality Distribution for Variant Sites"));
	table.add_row(vec!(img_str(&get_path("coverage_variants")), img_str(&get_path("quality_variants"))));
	body.push(Content::Table(table));
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("Filter Criteria Distribution"));
	table = HtmlTable::new("hor-zebra");
	table.add_header(vec!("Phred scale strand bias estimated by Fisher's Exact Test"));
	table.add_row(vec!(img_str(&get_path("fs_variant"))));
	body.push(Content::Table(table));
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	table = HtmlTable::new("green");
	table.add_header(vec!("Quality by Depth for Variant Alleles", "Quality by Depth for Non-Variant Alleles"));
	table.add_row(vec!(img_str(&get_path("qd_variant")), img_str(&get_path("qd_nonvariant"))));
	body.push(Content::Table(table));
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	table = HtmlTable::new("hor-zebra");
	table.add_header(vec!("RMS Mapping Quality for Variant Alleles", "RMS Mapping Qality for Non-Variant Alleles"));
	table.add_row(vec!(img_str(&get_path("rmsmq_variant")), img_str(&get_path("rmsmq_nonvariant"))));
	body.push(Content::Table(table));
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("Variant Types"));
	let (t1, t2) = make_variant_type_tables(json, vsumm)?;
	body.push(t1);
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("Ti / Tv Ratio"));
	body.push(t2);	
	let(t, total, _) = prep_hist_vec(&json.coverage().variant);
	let mut tmp = 0;
	for(ix, x) in t.iter() {
		tmp += *x;
		if tmp >= total >> 1 { 
			vsumm.med_cov_var_passed = *ix;
			break;
		}
	}
	Ok(body)
}

fn create_meth_report_body(project: &str, bc: &str, dir: &Path, json: &CallJson, msumm: &mut MethSummary) -> Result<HtmlElement, String> {
	let mut img_dir = dir.to_owned();
	img_dir.push("images");
	let mut body = new_body(project, bc, "methylation");
	body.push_element(make_section("CpG Counts"));
	body.push(make_cpg_count_table(json, msumm)?);
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("CpG Coverage"));
	let get_path = |name: &str| {
		let mut tp = img_dir.clone();
		tp.push(format!("{}_{}.png", bc, name).as_str());
		tp
	};
	let img_str = |p: &Path| {
		let fname = p.file_name().expect("Missing filename").to_string_lossy();
		format!("<img src=\"images/{}\" alt=\"{}\">", fname, fname)	
	};
	let mut table = HtmlTable::new("green");
	table.add_header(vec!("Coverage Distribution for Reference CpGs", "Informative Read Distribution for Reference CpGs"));
	table.add_row(vec!(img_str(&get_path("coverage_ref_cpg")), img_str(&get_path("coverage_ref_cpg_inf"))));
	body.push(Content::Table(table));
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	table = HtmlTable::new("hor-zebra");
	table.add_header(vec!("Quality Distribution for Reference CpGs", "Quality Distribution for Non Reference CpGs"));
	table.add_row(vec!(img_str(&get_path("quality_ref_cpg")), img_str(&get_path("quality_non_ref_cpg"))));
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("CpG Methylation Distribution"));
	body.push(Content::Table(table));
	table = HtmlTable::new("green");
	table.add_header(vec!("CpG Methylation Distribution"));
	table.add_row(vec!(img_str(&get_path("methylation_levels"))));
	body.push(Content::Table(table));
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("CpG Methylation Profiles"));
	body.push(make_cpg_meth_profile_table(json, msumm)?);
	let(t, total, _) = prep_hist_vec(&json.coverage().ref_cpg);
	let mut tmp = 0;
	for(ix, x) in t.iter() {
		tmp += *x;
		if tmp >= total >> 1 { 
			msumm.med_cpg_cov = *ix;
			break;
		}
	}
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


// Strategy!
//
// For each of the three report we create a new LatexSection.  We then lock latex_doc and check if there is already a SectionArray for this barcode. 
// If not we create it, and then we add the new section to the array.  We give the tags for the Sections so that they will sort in the desired order 
// for the output
//

fn create_mapping_latex_section(bc: &str, json: &CallJson) -> Result<LatexSection, String> {
	let mut img_dir = PathBuf::from_str(bc).expect("Couldn't get Path from barcode");
	img_dir.push("images");
	let mut sec = LatexSection::new("A");
	sec.push_string(format!("\\subsection{{Alignment \\& Coverage Report for {}}}", bc));
	sec.push_str("\\subsubsection{{Read Level Counts}}");
	sec.push(make_read_level_latex_tab(json)?);
	sec.push_str("\\subsubsection{{Base Level Counts}}");
	sec.push(make_base_level_latex_tab(json)?);
	sec.push_str("\\subsubsection{{Coverage Distribution}}");
	sec.push_string(format!("\\includegraphics[width=12cm]{{{}}}", img_dir.join(format!("{}_coverage_all", bc).as_str()).display()));
	sec.push_str("\\subsubsection{{Quality Distribution}}");
	sec.push_string(format!("\\includegraphics[width=12cm]{{{}}}", img_dir.join(format!("{}_quality_all", bc).as_str()).display()));
	sec.push_str("\\subsubsection{{GC/Coverage Heatmap}}");
	sec.push_string(format!("\\includegraphics[width=12cm]{{{}}}", img_dir.join(format!("{}_gc_coverage", bc).as_str()).display()));
	sec.push_str("\\subsubsection{{Non-Conversion at Non-CpG Sites}}");
	sec.push_string(format!("\\includegraphics[width=12cm]{{{}}}", img_dir.join(format!("{}_non_cpg_read_profile", bc).as_str()).display()));	
	Ok(sec)
}

fn create_variant_latex_section(bc: &str, json: &CallJson) -> Result<LatexSection, String> {
	let mut img_dir = PathBuf::from_str(bc).expect("Couldn't get Path from barcode");
	img_dir.push("images");
	let set_img = |nm: &str, sec: &mut LatexSection| {
		sec.push_string(format!("\\includegraphics[width=12cm]{{{}}}", img_dir.join(format!("{}_{}", bc, nm).as_str()).display()));		
	};
	let mut sec = LatexSection::new("B");
	sec.push_string(format!("\\subsection{{Variant Calling Report for {}}}", bc));
	sec.push_str("\\subsubsection{Variant Counts}");
	sec.push(make_variant_count_latex_tab(json)?);
	sec.push_str("\\subsubsection{VCF Filter Stats}");
	sec.push(make_vcf_filter_stats_latex_tab(json)?);
	sec.push_str("\\subsubsection{Coverage Distribution for Variant Sites}");
	set_img("coverage_variants", &mut sec);
	sec.push_str("\\subsubsection{Quality Distribution for Variant Sites}");
	set_img("quality_variants", &mut sec);
	sec.push_str("\\subsubsection{Filter Criteria Distributions}");
	sec.push_str("\\subsubsection*{Phred Scale Strand Bias (Fisher's Exact Test)}");
	set_img("fs_variant", &mut sec);
	sec.push_str("\\subsubsection*{Quality by Depth for Variant Alleles}");
	set_img("qd_variant", &mut sec);
	sec.push_str("\\subsubsection*{Quality by Depth for Non-Variant Alleles}");
	set_img("qd_nonvariant", &mut sec);
	sec.push_str("\\subsubsection*{RMS Mapping Quality for Variant Alleles}");
	set_img("rmsmq_variant", &mut sec);
	sec.push_str("\\subsubsection*{RMS Mapping Quality for Non-Variant Alleles}");
	set_img("rmsmq_nonvariant", &mut sec);
	let (t1, t2) = make_variant_type_latex_tabs(json)?;
	sec.push_str("\\subsubsection{Variant Types}");
	sec.push(t1);
	sec.push_str("\\subsubsection{Ti / Tv Ratio}");
	sec.push(t2);
	Ok(sec)
}

fn create_meth_latex_section(bc: &str, json: &CallJson) -> Result<LatexSection, String> {
	let mut img_dir = PathBuf::from_str(bc).expect("Couldn't get Path from barcode");
	img_dir.push("images");
	let set_img = |nm: &str, sec: &mut LatexSection| {
		sec.push_string(format!("\\includegraphics[width=12cm]{{{}}}", img_dir.join(format!("{}_{}", bc, nm).as_str()).display()));		
	};
	let mut sec = LatexSection::new("C");
	sec.push_string(format!("\\subsection{{Methylation Report for {}}}", bc));
	sec.push_str("\\subsubsection{CpG Counts}");
	sec.push(make_cpg_count_latex_tab(json)?);
	sec.push_str("\\subsubsection{Coverage Distribution for Reference CpGs}");
	set_img("coverage_ref_cpg", &mut sec);
	sec.push_str("\\subsubsection{Informative Read Distribution for Reference CpGs}");
	set_img("coverage_ref_cpg_inf", &mut sec);
	sec.push_str("\\subsubsection{Quality Distribution for Reference CpGs}");
	set_img("quality_ref_cpg", &mut sec);
	sec.push_str("\\subsubsection{Quality Distribution for Non Reference CpGs}");
	set_img("quality_non_ref_cpg", &mut sec);
	sec.push_str("\\subsubsection{CpG Methylation Distribution}");
	set_img("methylation_levels", &mut sec);
	sec.push_str("\\subsubsection{CpG Methylation Profiles}");
	sec.push(make_cpg_meth_profile_latex_tab(json)?);
	Ok(sec)
}

fn get_section_array_for_bc<'a>(ldoc: &'a mut LatexBare, bc: &str) -> Result<&'a mut SectionArray, String> {
	if ldoc.find_section(bc).is_none() {
		let mut  s = LatexSection::new(bc);
		s.push_string(format!("\\newpage\n\\section{{Report for Sample {}}}", bc));
		s.push(LatexContent::SecArray(SectionArray::new()));
		ldoc.push_section(s)?;
	}
	let sec = ldoc.find_section(bc).expect("Couldn't find LatexSection");
	let mut sa = None;
	for c in sec.content().iter_mut() {
		if let LatexContent::SecArray(ref mut s) = c { 
			sa = Some(s);
			break;
		}
	}
	sa.ok_or_else(|| format!("Could not find SectionArray for Sample {}", bc))
}

fn create_mapping_report(bc: &str, dir: &Path, project: &str, call_json: &CallJson, summary: Arc<Mutex<HashMap<String, CallSummary>>>, latex_doc: Arc<Mutex<LatexBare>>) -> Result<(), String> {
	let path: PathBuf = [dir, Path::new(format!("{}_mapping_coverage.html", bc).as_str())].iter().collect();
	let mut html = new_page(&path)?;
	let mut map_summ = MapSummary::new();
	html.push_element(create_mapping_report_body(project, bc, dir, call_json, &mut map_summ)?);	
	let mut shash = summary.lock().expect("Couldn't lock CallSummary");
	shash.get_mut(&bc.to_owned()).expect("Couldn't find CallSummary for sample").map = Some(map_summ);
	let sec = create_mapping_latex_section(bc, call_json)?;
	if let Ok(mut ldoc) = latex_doc.lock() { 
		let sa = get_section_array_for_bc(&mut ldoc, bc)?;
		sa.push(sec);
		Ok(())
	} else { Err("Couldn't obtain lock on latex doc".to_string()) }
}

fn create_variant_report(bc: &str, dir: &Path, project: &str, call_json: &CallJson, summary: Arc<Mutex<HashMap<String, CallSummary>>>, latex_doc: Arc<Mutex<LatexBare>>) -> Result<(), String> {
	let path: PathBuf = [dir, Path::new(format!("{}_variants.html", bc).as_str())].iter().collect();
	let mut var_summ = VarSummary::new();
	let mut html = new_page(&path)?;
	html.push_element(create_variant_report_body(project, bc, dir, call_json, &mut var_summ)?);	
	let mut shash = summary.lock().expect("Couldn't lock CallSummary");
	shash.get_mut(&bc.to_owned()).expect("Couldn't find CallSummary for sample").var = Some(var_summ);
	let sec = create_variant_latex_section(bc, call_json)?;
	if let Ok(mut ldoc) = latex_doc.lock() { 
		let sa = get_section_array_for_bc(&mut ldoc, bc)?;
		sa.push(sec);
		Ok(())
	} else { Err("Couldn't obtain lock on latex doc".to_string()) }
}

fn create_meth_report(bc: &str, dir: &Path, project: &str, call_json: &CallJson, summary: Arc<Mutex<HashMap<String, CallSummary>>>, latex_doc: Arc<Mutex<LatexBare>>) -> Result<(), String> {
	let path: PathBuf = [dir, Path::new(format!("{}_methylation.html", bc).as_str())].iter().collect();
	let mut html = new_page(&path)?;
	let mut meth_summ = MethSummary::new();
	html.push_element(create_meth_report_body(project, bc, dir, call_json, &mut meth_summ)?);	
	let mut shash = summary.lock().expect("Couldn't lock CallSummary");
	shash.get_mut(&bc.to_owned()).expect("Couldn't find CallSummary for sample").meth = Some(meth_summ);
	let sec = create_meth_latex_section(bc, call_json)?;
	if let Ok(mut ldoc) = latex_doc.lock() { 
		let sa = get_section_array_for_bc(&mut ldoc, bc)?;
		sa.push(sec);
		Ok(())
	} else { Err("Couldn't obtain lock on latex doc".to_string()) }
}

fn create_summary(dir: &Path, summary: Arc<Mutex<HashMap<String, CallSummary>>>, latex_doc: Arc<Mutex<LatexBare>>) -> Result<(), String> {
	let mut path = dir.to_owned();
	path.push("index.html");
	let mut html = HtmlPage::new(&path)?;
	let mut head_element = HtmlElement::new("HEAD", None, true);
	let mut style_element = HtmlElement::new("STYLE", Some("TYPE=\"text/css\""), true);
	style_element.push_str("<!--\n@import url(\"../css/style.css\");\n-->");
	head_element.push_element(style_element);
	html.push_element(head_element);
	let mut body = HtmlElement::new("BODY", None, true);
	let mut table = HtmlTable::new("hor-zebra");
	let f = |x| {
		if x > 1_000_000_000 { format!("{:.2} Tb", (x as f64) / 1_000_000_000.0)}
		else if x > 1_000_000 { format!("{:.2} Mb", (x as f64) / 1_000_000.0)}
		else if x > 1_000 { format!("{:.2} Kb", (x as f64) / 1_000.0)}
		else {format!("{}", x)}
	};
	table.add_header(vec!(
		"Sample", "Aligned", "Uniquely Aligned", "Passed", "GC Depth corr.", "Variants", "Passed Variants", "Med. Cov. Passed Variants",
		"Ti/Tv Ratio", "Med. CpG Meth.", "Med. CpG Cov.", "Passed CpGs", "Reports"));
	let mut ltable1 = LatexTable::new();
	let mut ltable2 = LatexTable::new();
	let mut ltable3 = LatexTable::new();
	ltable1.add_header(vec!("Sample", "Aligned", "Uniquely Aligned", "Passed", "GC Depth Corr."));
	ltable2.set_col_desc("|m{1.6cm}|m{3cm}|m{3cm}|m{2.5cm}|");
	ltable2.add_header(vec!("Variants", "Passed Variants", "Median Cov. of Passed Variants", "Ti/Tv Ratio"));
	ltable3.set_col_desc("|m{2.3cm}|m{2.2cm}|m{1.6cm}|");
	ltable3.add_header(vec!("Median CpG Meth.", "Median CpG Cov.", "Passed CpGs"));
	if let Ok(sum_vec) = summary.lock() {
		for (bc, s) in sum_vec.iter() {
			let mut row = Vec::new();
			let mut lrow1 = Vec::new();
			let mut lrow2 = Vec::new();
			let mut lrow3 = Vec::new();
			let  map_summ = s.map.as_ref().expect("Empty Map Summary");
			let  var_summ = s.var.as_ref().expect("Empty Variant Summary");
			let  meth_summ = s.meth.as_ref().expect("Empty Meth Summary");
			row.push(bc.to_string());
			lrow1.push(bc.to_string());
			row.push(f(map_summ.aligned));
			lrow1.push(f(map_summ.aligned));
			row.push(format!("{} ({:.2} %)", f(map_summ.unique), pct(map_summ.unique, map_summ.aligned)));
			lrow1.push(format!("{} ({:.2} %)", f(map_summ.unique), pct(map_summ.unique, map_summ.aligned)));
			row.push(format!("{} ({:.2} %)", f(map_summ.passed), pct(map_summ.passed, map_summ.aligned)));
			lrow1.push(format!("{} ({:.2} %)", f(map_summ.passed), pct(map_summ.passed, map_summ.aligned)));
			row.push(format!("{:.2}", map_summ.gc_correlation));
			lrow1.push(format!("{:.2}", map_summ.gc_correlation));
			ltable1.add_row(lrow1);
			row.push(format!("{:.3e}", var_summ.variants));
			lrow2.push(format!("{:.3e}", var_summ.variants));
			row.push(format!("{:.3e} ({:.2} %)", var_summ.variants_passed, pct(var_summ.variants_passed, var_summ.variants)));
			lrow2.push(format!("{:.3e} ({:.2} %)", var_summ.variants_passed, pct(var_summ.variants_passed, var_summ.variants)));
			row.push(format!("{}x", var_summ.med_cov_var_passed));
			lrow2.push(format!("{}x", var_summ.med_cov_var_passed));
			row.push(format!("{:.2}", var_summ.ti_tv_ratio));
			lrow2.push(format!("{:.2}", var_summ.ti_tv_ratio));
			ltable2.add_row(lrow2);
			row.push(format!("{:.2}", meth_summ.med_cpg_meth));
			lrow3.push(format!("{:.2}", meth_summ.med_cpg_meth));
			row.push(format!("{}x", meth_summ.med_cpg_cov));
			lrow3.push(format!("{}x", meth_summ.med_cpg_cov));
			row.push(format!("{:.3e}", meth_summ.passed_cpgs));
			lrow3.push(format!("{:.3e}", meth_summ.passed_cpgs));
			let mut link1 = HtmlElement::new("a", Some(format!("class=\"link\" href=\"{}/{}_mapping_coverage.html\"", bc, bc).as_str()), true);
			link1.push_str("&#187 Alignments & Coverage");
			let mut link2 = HtmlElement::new("a", Some(format!("class=\"link\" href=\"{}/{}_variants.html\"", bc, bc).as_str()), true);
			link2.push_str("&#187 Variants");
			let mut link3 = HtmlElement::new("a", Some(format!("class=\"link\" href=\"{}/{}_methylation.html\"", bc, bc).as_str()), true);
			link3.push_str("&#187 Methylation");
			row.push(format!("{}<BR>{}<BR>{}<BR>", link1, link2, link3));
			table.add_row(row);
			ltable3.add_row(lrow3);
		}
	} else { return Err("Couldn't obtain lock on sample summary".to_string()); }
	body.push(Content::Table(table));
	html.push_element(body);	
		if let Ok(mut ldoc) = latex_doc.lock() { 
		ldoc.push(LatexContent::Text("\\section{{Sample Summaries}}".to_string()));
		ldoc.push(LatexContent::Text("\\subsection{{Alignment \\& Coverage}}".to_string()));
		ldoc.push(LatexContent::Table(ltable1));
		ldoc.push(LatexContent::Text("\\subsection{{Variant Calling}}".to_string()));
		ldoc.push(LatexContent::Table(ltable2));
		ldoc.push(LatexContent::Text("\\subsection{{Methylation}}".to_string()));
		ldoc.push(LatexContent::Table(ltable3));
		Ok(())
	} else { Err("Couldn't obtain lock on latex doc".to_string()) }
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
		CallJob::MappingReport => create_mapping_report(bc, bc_dir, project, cj, job.summary, job.latex_doc),
		CallJob::VariantReport => create_variant_report(bc, bc_dir, project, cj, job.summary, job.latex_doc),
		CallJob::MethylationReport => create_meth_report(bc, bc_dir, project, cj, job.summary, job.latex_doc),
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

fn prepare_jobs(svec: &[CallJsonFiles], project: &str, summary: Arc<Mutex<HashMap<String, CallSummary>>>, latex_doc: Arc<Mutex<LatexBare>>) -> Vec<ReportJob> {
	let mut v = Vec::new();
	for cjson in svec.iter() {
		let call_json = Arc::new(RwLock::new(None));
		let load_json = LoadCallJson{path: cjson.json_file.clone(), call_json: call_json.clone()};
		let ld_json_ix = v.len();
		v.push(ReportJob::new(&cjson.barcode, project, &cjson.bc_dir, RepJob::CallJson(load_json)));
		for job_type in CallJob::iter() {
			let mk_graph = MakeCallJob{job_type, depend: ld_json_ix, call_json: call_json.clone(), summary: summary.clone(), latex_doc: latex_doc.clone()};
			v.push(ReportJob::new(&cjson.barcode, project, &cjson.bc_dir, RepJob::CallJob(mk_graph)));
		}
	}
	for (ix, job) in v.iter_mut().enumerate() { job.ix = ix }
	v
}

pub fn make_call_report(sig: Arc<AtomicUsize>, outputs: &[PathBuf], project: Option<String>, css: &Path, n_cores: usize, svec: Vec<CallJsonFiles>) -> Result<Option<Box<dyn BufRead>>, String> {
	utils::check_signal(Arc::clone(&sig))?;
	let project = project.unwrap_or_else(|| "gemBS".to_string());
	let report_tex_path = outputs.first().expect("No output files for call report");
	let output_dir = report_tex_path.parent().expect("No parent directory found for call report");
	// Set up worker threads	
	// Maximum parallel jobs that we could do if there were enough cores is 18 * the number of samples (18 images per sample)
	let n_dsets = svec.len() * 18;
	let n_workers = if n_cores > n_dsets { n_dsets } else { n_cores };
	let mut shash = HashMap::new();
	for cjson in svec.iter() { shash.insert(cjson.barcode.clone(), CallSummary::new()); }
	let summary = Arc::new(Mutex::new(shash));
	let latex_doc = Arc::new(Mutex::new(LatexBare::new(&report_tex_path)?));
	let mut job_vec = prepare_jobs(&svec, &project, summary.clone(), latex_doc.clone());
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
		create_summary(output_dir, summary, latex_doc)?; 
		make_map_report::copy_css(output_dir, css)?;
		Ok(None) 

	}
}

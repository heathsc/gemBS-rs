use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, mpsc};
use std::{fs, thread, time};
use std::collections::HashMap;
use std::str::FromStr;

use plotters::prelude::*;

use crate::scheduler::report::SampleJsonFiles;
use crate::scheduler::call;
use crate::common::utils;
use crate::common::json_map_stats::{MapJson, BaseCounts, Counts, Count, Paired, New};
use crate::common::html_utils::*;

struct Worker {
	handle: thread::JoinHandle<Result<(), String>>,
	tx: mpsc::Sender<Option<ReportJob>>,
	ix: usize,
}

struct ReportJob {
	jfiles: SampleJsonFiles,
	project: Option<String>,
	mapq_threshold: usize,
}

fn make_title(title: String) -> HtmlElement {
	let mut utitle = HtmlElement::new("U", None, true);
	utitle.push_string(title);
	let mut t = HtmlElement::new("H1", Some("id=\"title\""), true);
	t.push_element(utitle);
	t
}

fn make_section(s: &str) -> HtmlElement {
	let mut t = HtmlElement::new("H1", Some("id=\"section\""), true);
	t.push_str(s);
	t
}

fn pct(a: usize, b: usize) -> f64 {
	if b > 0 { 100.0 * (a as f64) / (b as f64) }
	else { 0.0 }	
}

fn make_paired_row(x: Counts, total: Counts, s: &str) -> Vec<String> {
	let mut row = vec!(s.to_owned());
	row.push(format!("{}", x[0] + x[1]));
	row.push(format!("{:.2} %", pct(x[0] + x[1], total[0] + total[1])));
	row.push(format!("{}", x[0]));
	row.push(format!("{:.2} %", pct(x[0], total[0])));
	row.push(format!("{}", x[1]));
	row.push(format!("{:.2} %", pct(x[1], total[1])));
	row
}

fn make_single_row(x: Count, total: Count, s: &str) -> Vec<String> {
	let mut row = vec!(s.to_owned());
	row.push(format!("{}", x[0]));
	row.push(format!("{:.2} %", pct(x[0], total[0])));
	row
}

fn make_reads_table(json: &MapJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("hor-zebra");
	let mut hdr = vec!("Concept", "Total Reads", "%");
	match json {
		MapJson::Paired(x) | MapJson::Unknown(x) => {
			hdr.extend(&["Pair One Reads", "%", "Pair Two Reads", "%"]);
			let reads = x.reads();
			let total = reads.get_total();
			table.add_row(make_paired_row(total, total, "Sequenced Reads"));
			table.add_row(make_paired_row(reads.general, total, "General Reads"));
			if let Some(ct) = reads.sequencing_control { table.add_row(make_paired_row(ct, total, "Control Sequence Reads")); }
			if let Some(ct) = reads.under_conversion_control { table.add_row(make_paired_row(ct, total, "Underconversion Control Sequence Reads")); }
			if let Some(ct) = reads.over_conversion_control { table.add_row(make_paired_row(ct, total, "Overconversion Control Sequence Reads")); }
			table.add_row(make_paired_row(reads.unmapped, total, "Unmapped Reads"));
			if let Some(bs_reads) = x.bs_reads() {
				table.add_row(make_paired_row(bs_reads.c2t, total, "Bisulfite Reads C2T"));
				table.add_row(make_paired_row(bs_reads.g2a, total, "Bisulfite Reads G2A"));
			}
		},
		MapJson::Single(x) => {
			let reads = x.reads();
			let total = reads.get_total();
			table.add_row(make_single_row(reads.get_total(), total, "Sequenced Reads"));		
			table.add_row(make_single_row(reads.general, total, "General Reads"));
			if let Some(ct) = reads.sequencing_control { table.add_row(make_single_row(ct, total, "Control Sequence Reads")); }
			if let Some(ct) = reads.under_conversion_control { table.add_row(make_single_row(ct, total, "Underconversion Control Sequence Reads")); }
			if let Some(ct) = reads.over_conversion_control { table.add_row(make_single_row(ct, total, "Overconversion Control Sequence Reads")); }
			table.add_row(make_single_row(reads.unmapped, total, "Unmapped Reads"));
			if let Some(bs_reads) = x.bs_reads() {
				table.add_row(make_single_row(bs_reads.c2t, total, "Bisulfite Reads C2T"));
				table.add_row(make_single_row(bs_reads.g2a, total, "Bisulfite Reads G2A"));
			}
		},
	}
	table.add_header(hdr);
	Ok(Content::Table(table))
}

fn make_bases_table(json: &MapJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("hor-zebra");
	let mut hdr = vec!("Concept", "Total Bases", "%");
	match json {
		MapJson::Paired(x) | MapJson::Unknown(x) => {
			hdr.extend(&["Pair One Bases", "%", "Pair Two Bases", "%"]);
			let bc = x.base_counts().overall;
			let total = bc.get_total();
			table.add_row(make_paired_row(bc.a, total, "Base Counts Overall A"));
			table.add_row(make_paired_row(bc.c, total, "Base Counts Overall C"));
			table.add_row(make_paired_row(bc.g, total, "Base Counts Overall G"));
			table.add_row(make_paired_row(bc.t, total, "Base Counts Overall T"));
			table.add_row(make_paired_row(bc.n, total, "Base Counts Overall N"));
		},
		MapJson::Single(x) => {
			let bc = x.base_counts().overall;
			let total = bc.get_total();
			table.add_row(make_single_row(bc.a, total, "Base Counts Overall A"));
			table.add_row(make_single_row(bc.c, total, "Base Counts Overall C"));
			table.add_row(make_single_row(bc.g, total, "Base Counts Overall G"));
			table.add_row(make_single_row(bc.t, total, "Base Counts Overall T"));
			table.add_row(make_single_row(bc.n, total, "Base Counts Overall N"));
		},
	}
	table.add_header(hdr);
	Ok(Content::Table(table))
}

fn trans_paired_hash(hr: &[HashMap<String, usize>; 2]) -> Result<(Vec<(usize, Counts)>, Counts), String> {
	let mut t = HashMap::new();
	for (ix, y) in hr[0].iter() { 
		let c = Counts([*y, 0]);
		t.insert(<usize>::from_str(ix).map_err(|e| format!("{}", e))?, c);
	}
	for (ix, y) in hr[1].iter() { 
		let c = Counts([0, *y]);
		*(t.entry(<usize>::from_str(ix).map_err(|e| format!("{}", e))?).or_insert(Counts([0;2]))) += c; 
	}
	let mut total = Counts::new();
	let mut rl = Vec::new();
	for (ix, y) in t.iter() {
		rl.push((*ix, *y));
		total += *y;
	}
	rl.sort_by(|a, b| a.0.cmp(&b.0));
	Ok((rl, total))	
}

fn trans_single_hash(hr: &HashMap<String, usize>) -> Result<(Vec<(usize, Count)>, Count), String> {
	let mut rl = Vec::new();
	let mut total = Count::new();
	for (ix, y) in hr.iter() { 
		let c = Count([*y]);
		total += c;	
		rl.push((<usize>::from_str(ix).map_err(|e| format!("{}", e))?, c));
	}
	rl.sort_by(|a, b| a.0.cmp(&b.0));
	Ok((rl, total))
}

fn make_read_length_table(json: &MapJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("hor-zebra");
	let mut hdr = vec!("Read Length", "Total Reads", "%");
	match json {
		MapJson::Paired(x) | MapJson::Unknown(x) => {
			hdr.extend(&["Read One", "%", "Read Two", "%"]);			
			let (rl, total) = trans_paired_hash(x.read_len())?;
			for (x, y) in rl.iter() { table.add_row(make_paired_row(*y, total, format!("{}", x).as_str())); }
		},
		MapJson::Single(x) => {
			let (rl, total) = trans_single_hash(x.read_len())?;
			for (x, y) in rl.iter() { table.add_row(make_single_row(*y, total, format!("{}", x).as_str())); }
		},
	}
	table.add_header(hdr);
	Ok(Content::Table(table))
}

fn make_mismatch_table(json: &MapJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("green");
	let mut hdr = vec!("Number of Mismatches", "Total Reads", "%");
	match json {
		MapJson::Paired(x) | MapJson::Unknown(x) => {
			hdr.extend(&["Read One", "%", "Read Two", "%"]);			
			let (rl, total) = trans_paired_hash(x.mismatch())?;
			for (x, y) in rl.iter() { table.add_row(make_paired_row(*y, total, format!("{}", x).as_str())); }
		},
		MapJson::Single(x) => {
			let (rl, total) = trans_single_hash(x.mismatch())?;
			for (x, y) in rl.iter() { table.add_row(make_single_row(*y, total, format!("{}", x).as_str())); }
		},
	}
	table.add_header(hdr);
	Ok(Content::Table(table))
}

fn make_unique_table(mapq_threshold: usize, json: &MapJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("green");
	table.add_header(vec!("Concept", "Value"));
	let (ct, tot) = json.get_unique(mapq_threshold);
	let mut row = vec!("Unique Fragments".to_string());
	row.push(format!("{}", ct));
	table.add_row(row);
	let mut row = vec!("% Unique".to_string());
	row.push(format!("{:.2} %", pct(ct, tot)));
	table.add_row(row);	
	Ok(Content::Table(table))
}

fn make_conversion_table(json: &MapJson) -> Result<Content, String> {
	let mut table = HtmlTable::new("green");
	table.add_header(vec!("Bisulfite Conversion Type", "Conversion Rate"));
	let (ct1, ct2) = json.get_conversion_counts();	
	let conv = if let Some(x) = call::calc_conversion(&ct1) { format!("{:.4}", x) } else { "NA".to_string() };
	table.add_row(vec!("Conversion Rate of non-methylated Cytosines".to_string(), conv));
	let conv = if let Some(x) = call::calc_conversion(&ct2) { format!("{:.4}", x) } else { "NA".to_string() };
	table.add_row(vec!("Conversion Rate of methylated Cytosines".to_string(), conv));
	Ok(Content::Table(table))
}

fn make_correct_pairs_table(paired: &Paired) -> Result<Content, String> {
	let mut table = HtmlTable::new("hor-zebra");
	table.add_header(vec!("Concept", "Read Pairs"));
	let corr_pairs = format!("{}", paired.correct_pairs());
	table.add_row(vec!("Correct Pairs".to_string(), corr_pairs));	
	Ok(Content::Table(table))
}

fn make_mapq_table(path: &Path) -> Result<Content, String> {
	let mut table = HtmlTable::new("green");
	table.add_header(vec!("Mapping Quality Histogram"));
	let fname = path.file_name().expect("Missing filename").to_string_lossy();
	table.add_row(vec!(format!("<img src=\"images/{}\" alt=\"{}\">", fname, fname)));	
	Ok(Content::Table(table))
}

fn make_isize_table(path: &Path) -> Result<Content, String> {
	let mut table = HtmlTable::new("green");
	table.add_header(vec!("Insert Size Histogram"));
	let fname = path.file_name().expect("Missing filename").to_string_lossy();
	table.add_row(vec!(format!("<img src=\"images/{}\" alt=\"{}\">", fname, fname)));	
	Ok(Content::Table(table))
}

fn create_mapq_hist(path: &Path, json: &MapJson) -> Result<(), Box<dyn std::error::Error>> {
	let hist_mapq = json.get_mapq_hist();
	let max = *hist_mapq.iter().max().expect("MapQ histogram empty") as f64;
	let len = hist_mapq.len()  as f64;	
    let root = BitMapBackend::new(path, (1024, 640)).into_drawing_area();
	root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(60)
        .margin(5)
        .caption("MapQ Histogram", ("sans-serif", 22.0).into_font())
        .build_ranged(-0.5..len + 0.5, 0.0..max)?;

    chart
        .configure_mesh()
//        .disable_x_mesh()
//        .disable_y_mesh()
        .line_style_1(&WHITE.mix(0.3))
        .y_desc("Fragments")
        .x_desc("MapQ")
		.y_label_formatter(&|y| format!("{:e}", y))
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;

    chart.draw_series(
		hist_mapq.iter().enumerate().map(|(x, y)| {
			Rectangle::new([((x as f64) - 0.5, 0.0), ((x as f64) + 0.5, (*y as f64))], BLUE.mix(0.4).filled())
		})
    )?;

	Ok(())
		
}

fn create_isize_hist(path: &Path, paired: &Paired) -> Result<(), Box<dyn std::error::Error>> {
	let tlen = paired.template_len();
	// Get the bottom 99.9% of read lengths
	// Put length histogram in a vector
	let mut tl = Vec::new();
	let mut total = 0;
	for (ix, y) in tlen.iter() { 
		total += y;	
		tl.push((<usize>::from_str(ix).map_err(|e| format!("{}", e))?, *y));
	}
	tl.sort_by(|a, b| a.0.cmp(&b.0));
	let mut tmp = 0;
	let mut max = 0;
	let thresh = (total as f64) * 0.999;
	let mut t = None;
	for (ix, y) in &tl {
		tmp += y;
		if *y > max { max = *y }
		if (tmp as f64) >= thresh {
			t = Some(ix);
			break;
		}
	}
	let lim = t.expect("No template lengths found");
    let root = BitMapBackend::new(path, (1024, 640)).into_drawing_area();
	root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(60)
        .margin(5)
        .caption("Insert Size Histogram", ("sans-serif", 22.0).into_font())
        .build_ranged(0..*lim, 0..max)?;
 
    chart
        .configure_mesh()
        .line_style_1(&WHITE.mix(0.3))
        .y_desc("Fragments")
        .x_desc("Insert Size (bp)")
		.y_label_formatter(&|y| format!("{:e}", *y as f64))
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;

    chart.draw_series(LineSeries::new(tl.iter().map(|(x, y)| (*x, *y)), &RED).point_size(3))?;
	Ok(())
		
}
fn create_sample_body(project: &str, bc: &str, ds: Option<&str>, mapq_threshold: usize, dir: &Path, json: &MapJson) -> Result<HtmlElement, String> {
	let (name, sample_report) = if let Some(s) = ds { (s, false) } else { (bc, true) };
	let mut img_dir = dir.to_owned();
	img_dir.push("images");
	let mut mapq_hist_png = img_dir.clone();
	mapq_hist_png.push(format!("{}_mapq.png", name).as_str());
	let mut isize_hist_png = None;
	create_mapq_hist(&mapq_hist_png, json).map_err(|e| format!("{}", e))?;
	let mut body = HtmlElement::new("BODY", None, true);
	let mut path = HtmlElement::new("P", Some("id=\"path\""), true);
	if sample_report { path.push_string(format!("/{}/{}", project, bc)); }
	else { path.push_string(format!("/{}/{}/{}", project, bc, name)); }
	body.push(Content::Element(path));
	let mut back = HtmlElement::new("B", None, true);
	back.push_str("BACK");
	let mut back_link = HtmlElement::new("a", Some(format!("class=\"link\" href=\"{}.html\"", bc).as_str()), true);
	back_link.push_element(back);
	body.push_element(back_link);
	body.push_element(HtmlElement::new("BR", None, false));
	if sample_report { body.push_element(make_title(format!("SAMPLE {}", bc))); }
	else { body.push_element(make_title(format!("SAMPLE {} LANE {}", bc, name))); }
	body.push_element(make_section("Mapping Stats (Reads)"));
	body.push(make_reads_table(json)?);
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section(format!("Uniquely Mapping Fragments (MAPQ >= {})", mapq_threshold).as_str()));
	body.push(make_unique_table(mapq_threshold, json)?);
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("Mapping Stats (Reads)"));
	body.push(make_bases_table(json)?);	
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("Bisulfite Conversion Rate"));
	body.push(make_conversion_table(json)?);
	match json {
		MapJson::Paired(x) | MapJson::Unknown(x) => {
			body.push_element(HtmlElement::new("BR><BR><BR", None, false));
			body.push_element(make_section("Correct Pairs"));
			body.push(make_correct_pairs_table(x)?);	

			let mut tp = img_dir;
			tp.push(format!("{}_isize.png", name).as_str());
			create_isize_hist(&tp, x).map_err(|e| format!("{}", e))?;
			isize_hist_png = Some(tp);
		},
		_ => (),
	}
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("Mapping Quality"));
	body.push(make_mapq_table(&mapq_hist_png)?);
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("Read Lengths"));
	body.push(make_read_length_table(json)?);
	body.push_element(HtmlElement::new("BR><BR><BR", None, false));
	body.push_element(make_section("Mismatch Distribution"));
	body.push(make_mismatch_table(json)?);
	if let Some(x) = isize_hist_png { 
		body.push_element(HtmlElement::new("BR><BR><BR", None, false));
		body.push_element(make_section("Insert Size"));
		body.push(make_isize_table(&x)?);	
	}
	Ok(body)
}

fn create_sample_html(project: &str, bc: &str, ds: Option<&str>, mapq_threshold: usize, dir: &Path, json: &MapJson) -> Result<(), String> {

	let mut path = dir.to_owned();
	let name = if let Some(s) = ds { s } else { bc };
	path.push(format!("{}.html", name).as_str());
	let mut html = HtmlPage::new(&path)?;
	let mut head_element = HtmlElement::new("HEAD", None, true);
	let mut style_element = HtmlElement::new("STYLE", Some("TYPE=\"text/css\""), true);
	style_element.push_str("<!--\n@import url(\"../../css/style.css\");\n-->");
	head_element.push_element(style_element);
	html.push_element(head_element);
	html.push_element(create_sample_body(project, bc, ds, mapq_threshold, dir, json)?);
	Ok(())
}

fn create_sample_report(job: ReportJob) -> Result<(), String> {
	let project = job.project.unwrap_or_else(|| "gemBS".to_string());
	let jfiles = job.jfiles;
	debug!("Creating sample report for {}/{}", project, jfiles.barcode);
	let bc_dir = jfiles.bc_dir.expect("No parent directory given for sample report output");
	let mut mrg_json: Option<MapJson> = None;
	let n_files = jfiles.json_files.len();
	for (ds, path) in jfiles.json_files {
		let file = match fs::File::open(path.clone()) {
			Err(e) => panic!("Couldn't open {}: {}", path.to_string_lossy(), e),
			Ok(f) => f,
		};
		let reader = Box::new(BufReader::new(file));
		let json = MapJson::from_reader(reader).unwrap_or_else(|e| panic!("Couldn't parse JSON file {}: {}", path.to_string_lossy(), e));
		if n_files > 1 { create_sample_html(&project, &jfiles.barcode, Some(&ds), job.mapq_threshold, &bc_dir, &json)?; }
		mrg_json = if let Some(j) = mrg_json {
			Some(j.merge(json))
		} else { Some(json) }
	}
	create_sample_html(&project, &jfiles.barcode, None, job.mapq_threshold, &bc_dir, &mrg_json.expect("No merged JSON struct"))?;
	Ok(())
}

fn worker_thread(tx: mpsc::Sender<isize>, rx: mpsc::Receiver<Option<ReportJob>>, idx: isize) -> Result<(), String> {
	loop {
		match rx.recv() {
			Ok(Some(job)) => {
				if let Err(e) = create_sample_report(job) {
					error!("Error creating sample report: {}", e);
					tx.send(-(idx + 1)).expect("Error sending message to parent");
				} else {
					tx.send(idx).expect("Error sending message to parent");
				}
			},
			Ok(None) => {
				debug!("Map report thread {} received signal to shutdown", idx);
				break;
			}
			Err(e) => {
				error!("Map report thread {} received error: {}", idx, e);
				break;
			}
		}
	}
	debug!("Map report thread {} shutting down", idx);
	Ok(())
}

pub fn make_map_report(sig: Arc<AtomicUsize>, outputs: &[PathBuf], project: Option<String>, mapq_threshold: usize, n_cores: usize, mut svec: Vec<SampleJsonFiles>) -> Result<Option<Box<dyn BufRead>>, String> {
	utils::check_signal(Arc::clone(&sig))?;
	let output_dir = outputs.first().expect("No output files for map report").parent().expect("No parent directory found for map report");
	println!("Make map report for {:?}, n_cores: {} in dir: {}", project, n_cores, output_dir.to_string_lossy());
	// Set up worker threads
	let n_workers = if n_cores > svec.len() { svec.len() } else { n_cores };
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
		let worker_ix = {
			if svec.is_empty() { None }
			else { avail.pop() }
		};
		if let Some(idx) =  worker_ix {
			let rep_job = if let Some(jfiles) = svec.pop() { Some(ReportJob{jfiles, project: project.clone(), mapq_threshold}) }
			else { None };
			jobs.push(idx as isize);
			debug!("Sending map report job to worker {}", idx);			
			workers[idx].tx.send(rep_job).expect("Error sending new command to map report worker thread");
			match ctr_rx.try_recv() {
				Ok(x) if x >= 0 => {
					debug!("Job completion by worker thread {}", x);
					jobs.retain(|ix| *ix != x);
					avail.push(x as usize);
				},
				Ok(x) => {
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
							
		} else if !jobs.is_empty() {
			match ctr_rx.recv_timeout(time::Duration::from_millis(1000)) {
				Ok(x) if x >= 0 => {
					debug!("Job completion by worker thread {}", x);
					jobs.retain(|ix| *ix != x);
					avail.push(x as usize);
				},
				Ok(x) => {
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
		} else if svec.is_empty() {
			break;
		} else { thread::sleep(time::Duration::from_secs(5)) }
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
	else { Ok(None) }
}

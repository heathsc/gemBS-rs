use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::AtomicUsize;
use std::sync::{mpsc, Arc, Mutex};
use std::{fs, thread, time};

use plotters::prelude::*;
use plotters_bitmap::BitMapBackend;

use super::report_utils::*;
use crate::common::html_utils::*;
use crate::common::json_map_stats::{Count, Counts, MapJson, MapJsonType, New, Paired};
use crate::common::latex_utils::*;
use crate::common::utils;
use crate::scheduler::call;
use crate::scheduler::report::SampleJsonFiles;

pub fn make_title(title: String) -> HtmlElement {
    let mut utitle = HtmlElement::new("U", None, true);
    utitle.push_string(title);
    let mut t = HtmlElement::new("H1", Some("id=\"title\""), true);
    t.push_element(utitle);
    t
}

pub fn make_section(s: &str) -> HtmlElement {
    let mut t = HtmlElement::new("H1", Some("id=\"section\""), true);
    t.push_str(s);
    t
}

fn make_paired_row(x: Counts, total: Counts, s: &str) -> Vec<String> {
    let mut row = vec![s.to_owned()];
    row.push(format!("{}", x[0] + x[1]));
    row.push(format!("{:.2} %", pct(x[0] + x[1], total[0] + total[1])));
    row.push(format!("{}", x[0]));
    row.push(format!("{:.2} %", pct(x[0], total[0])));
    row.push(format!("{}", x[1]));
    row.push(format!("{:.2} %", pct(x[1], total[1])));
    row
}

fn make_single_row(x: Count, total: Count, s: &str) -> Vec<String> {
    let mut row = vec![s.to_owned()];
    row.push(format!("{}", x[0]));
    row.push(format!("{:.2} %", pct(x[0], total[0])));
    row
}

fn make_reads_tab<T: Table>(table: &mut T, json: &MapJson) {
    let mut hdr = vec!["Concept", "Total Reads", "%"];
    match json {
        MapJson::Paired(x) | MapJson::Unknown(x) => {
            hdr.extend(&["Pair One", "%", "Pair Two", "%"]);
            let reads = x.reads();
            let total = reads.get_total();
            table.add_row(make_paired_row(total, total, "Sequenced"));
            table.add_row(make_paired_row(reads.general, total, "General"));
            if let Some(ct) = reads.sequencing_control {
                table.add_row(make_paired_row(ct, total, "Control Seq."));
            }
            if let Some(ct) = reads.under_conversion_control {
                table.add_row(make_paired_row(ct, total, "Underconv. Seq."));
            }
            if let Some(ct) = reads.over_conversion_control {
                table.add_row(make_paired_row(ct, total, "Overconv. Seq."));
            }
            table.add_row(make_paired_row(reads.unmapped, total, "Unmapped"));
            if let Some(bs_reads) = x.bs_reads() {
                table.add_row(make_paired_row(bs_reads.c2t, total, "Bisulfite C2T"));
                table.add_row(make_paired_row(bs_reads.g2a, total, "Bisulfite G2A"));
            }
        }
        MapJson::Single(x) => {
            let reads = x.reads();
            let total = reads.get_total();
            table.add_row(make_single_row(reads.get_total(), total, "Sequenced"));
            table.add_row(make_single_row(reads.general, total, "General"));
            if let Some(ct) = reads.sequencing_control {
                table.add_row(make_single_row(ct, total, "Control Seq."));
            }
            if let Some(ct) = reads.under_conversion_control {
                table.add_row(make_single_row(ct, total, "Underconv. Seq."));
            }
            if let Some(ct) = reads.over_conversion_control {
                table.add_row(make_single_row(ct, total, "Overconv. Seq."));
            }
            table.add_row(make_single_row(reads.unmapped, total, "Unmapped"));
            if let Some(bs_reads) = x.bs_reads() {
                table.add_row(make_single_row(bs_reads.c2t, total, "Bisulfite C2T"));
                table.add_row(make_single_row(bs_reads.g2a, total, "Bisulfite G2A"));
            }
        }
    }
    table.add_header(hdr);
}

fn make_reads_table(json: &MapJson) -> Content {
    let mut table = HtmlTable::new("hor-zebra");
    make_reads_tab(&mut table, json);
    Content::Table(table)
}

fn make_reads_latex_tab(json: &MapJson) -> LatexContent {
    let mut table = LatexTable::new();
    make_reads_tab(&mut table, json);
    LatexContent::Table(table)
}

fn make_bases_tab<T: Table>(table: &mut T, json: &MapJson) {
    let mut hdr = vec!["Base", "Total Bases", "%"];
    match json {
        MapJson::Paired(x) | MapJson::Unknown(x) => {
            hdr.extend(&["Pair One", "%", "Pair Two", "%"]);
            let bc = x.base_counts().overall;
            let total = bc.get_total();
            table.add_row(make_paired_row(bc.a, total, "A"));
            table.add_row(make_paired_row(bc.c, total, "C"));
            table.add_row(make_paired_row(bc.g, total, "G"));
            table.add_row(make_paired_row(bc.t, total, "T"));
            table.add_row(make_paired_row(bc.n, total, "N"));
        }
        MapJson::Single(x) => {
            let bc = x.base_counts().overall;
            let total = bc.get_total();
            table.add_row(make_single_row(bc.a, total, "A"));
            table.add_row(make_single_row(bc.c, total, "C"));
            table.add_row(make_single_row(bc.g, total, "G"));
            table.add_row(make_single_row(bc.t, total, "T"));
            table.add_row(make_single_row(bc.n, total, "N"));
        }
    }
    table.add_header(hdr);
}

fn make_bases_table(json: &MapJson) -> Content {
    let mut table = HtmlTable::new("hor-zebra");
    make_bases_tab(&mut table, json);
    Content::Table(table)
}

fn make_bases_latex_tab(json: &MapJson) -> LatexContent {
    let mut table = LatexTable::new();
    make_bases_tab(&mut table, json);
    LatexContent::Table(table)
}

fn trans_paired_hash(
    hr: &[HashMap<String, usize>; 2],
) -> Result<(Vec<(usize, Counts)>, Counts), String> {
    let mut t = HashMap::new();
    for (ix, y) in hr[0].iter() {
        let c = Counts([*y, 0]);
        t.insert(<usize>::from_str(ix).map_err(|e| format!("{}", e))?, c);
    }
    for (ix, y) in hr[1].iter() {
        let c = Counts([0, *y]);
        *(t.entry(<usize>::from_str(ix).map_err(|e| format!("{}", e))?)
            .or_insert(Counts([0; 2]))) += c;
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

fn make_read_length_tab<T: Table>(table: &mut T, json: &MapJson) -> Result<(), String> {
    let mut hdr = vec!["Read Length", "Total Reads", "%"];
    match json {
        MapJson::Paired(x) | MapJson::Unknown(x) => {
            hdr.extend(&["Read One", "%", "Read Two", "%"]);
            let (rl, total) = trans_paired_hash(x.read_len())?;
            for (x, y) in rl.iter() {
                table.add_row(make_paired_row(*y, total, format!("{}", x).as_str()));
            }
        }
        MapJson::Single(x) => {
            let (rl, total) = trans_single_hash(x.read_len())?;
            for (x, y) in rl.iter() {
                table.add_row(make_single_row(*y, total, format!("{}", x).as_str()));
            }
        }
    }
    table.add_header(hdr);
    Ok(())
}

fn make_read_length_table(json: &MapJson) -> Result<Content, String> {
    let mut table = HtmlTable::new("hor-zebra");
    make_read_length_tab(&mut table, json)?;
    Ok(Content::Table(table))
}

fn make_read_length_latex_tab(json: &MapJson) -> Result<LatexContent, String> {
    let mut table = LatexTable::new();
    make_read_length_tab(&mut table, json)?;
    Ok(LatexContent::Table(table))
}

fn make_mismatch_tab<T: Table>(table: &mut T, json: &MapJson) -> Result<(), String> {
    let mut hdr = vec!["Number of Mismatches", "Total Reads", "%"];
    match json {
        MapJson::Paired(x) | MapJson::Unknown(x) => {
            hdr.extend(&["Read One", "%", "Read Two", "%"]);
            let (rl, total) = trans_paired_hash(x.mismatch())?;
            for (x, y) in rl.iter() {
                table.add_row(make_paired_row(*y, total, format!("{}", x).as_str()));
            }
        }
        MapJson::Single(x) => {
            let (rl, total) = trans_single_hash(x.mismatch())?;
            for (x, y) in rl.iter() {
                table.add_row(make_single_row(*y, total, format!("{}", x).as_str()));
            }
        }
    }
    table.add_header(hdr);
    Ok(())
}

fn make_mismatch_table(json: &MapJson) -> Result<Content, String> {
    let mut table = HtmlTable::new("green");
    make_mismatch_tab(&mut table, json)?;
    Ok(Content::Table(table))
}

fn make_mismatch_length_latex_tab(json: &MapJson) -> Result<LatexContent, String> {
    let mut table = LatexTable::new();
    table.set_col_desc("|m{2.4cm}|m{1.5cm}|m{1.5cm}|m{1.5cm}|m{1.5cm}|m{1.5cm}|m{1.5cm}|");
    make_mismatch_tab(&mut table, json)?;
    Ok(LatexContent::Table(table))
}

fn make_unique_tab<T: Table>(table: &mut T, mapq_threshold: usize, json: &MapJson) {
    table.add_header(vec!["Concept", "Value"]);
    let (ct, tot) = json.get_unique(mapq_threshold);
    let mut row = vec!["Unique Fragments".to_string()];
    row.push(format!("{}", ct));
    table.add_row(row);
    let mut row = vec!["% Unique".to_string()];
    row.push(format!("{:.2} %", pct(ct, tot)));
    table.add_row(row);
}

fn make_unique_table(mapq_threshold: usize, json: &MapJson) -> Content {
    let mut table = HtmlTable::new("green");
    make_unique_tab(&mut table, mapq_threshold, json);
    Content::Table(table)
}

fn make_unique_latex_tab(mapq_threshold: usize, json: &MapJson) -> LatexContent {
    let mut table = LatexTable::new();
    make_unique_tab(&mut table, mapq_threshold, json);
    LatexContent::Table(table)
}

fn make_conversion_tab<T: Table>(table: &mut T, json: &MapJson) {
    table.add_header(vec!["Bisulfite Conversion Type", "Conversion Rate"]);
    let (ct1, ct2) = json.get_conversion_counts();
    let conv = if let Some(x) = call::calc_conversion(&ct1) {
        format!("{:.4}", x)
    } else {
        "NA".to_string()
    };
    table.add_row(vec![
        "Conversion Rate of non-methylated Cytosines".to_string(),
        conv,
    ]);
    let conv = if let Some(x) = call::calc_conversion(&ct2) {
        format!("{:.4}", x)
    } else {
        "NA".to_string()
    };
    table.add_row(vec![
        "Conversion Rate of methylated Cytosines".to_string(),
        conv,
    ]);
}

fn make_conversion_table(json: &MapJson) -> Content {
    let mut table = HtmlTable::new("green");
    make_conversion_tab(&mut table, json);
    Content::Table(table)
}

fn make_conversion_latex_tab(json: &MapJson) -> LatexContent {
    let mut table = LatexTable::new();
    make_conversion_tab(&mut table, json);
    LatexContent::Table(table)
}

fn make_correct_pairs_table(paired: &Paired) -> Content {
    let mut table = HtmlTable::new("hor-zebra");
    table.add_header(vec!["Concept", "Read Pairs"]);
    let corr_pairs = format!("{}", paired.correct_pairs());
    table.add_row(vec!["Correct Pairs".to_string(), corr_pairs]);
    Content::Table(table)
}

fn make_mapq_table(path: &Path) -> Content {
    let mut table = HtmlTable::new("green");
    table.add_header(vec!["Mapping Quality Histogram"]);
    let fname = path
        .file_name()
        .expect("Missing filename")
        .to_string_lossy();
    table.add_row(vec![format!(
        "<img src=\"images/{}\" alt=\"{}\">",
        fname, fname
    )]);
    Content::Table(table)
}

fn make_isize_table(path: &Path) -> Content {
    let mut table = HtmlTable::new("green");
    table.add_header(vec!["Insert Size Histogram"]);
    let fname = path
        .file_name()
        .expect("Missing filename")
        .to_string_lossy();
    table.add_row(vec![format!(
        "<img src=\"images/{}\" alt=\"{}\">",
        fname, fname
    )]);
    Content::Table(table)
}

fn make_links_table(ds: &[&str]) -> Content {
    let mut table = HtmlTable::new("hor-zebra");
    table.add_header(vec!["Lane Reports"]);
    for d in ds.iter() {
        let mut link = HtmlElement::new(
            "a",
            Some(format!("class=\"link\" href=\"{}.html\"", d).as_str()),
            true,
        );
        link.push_str(d);
        table.add_row(vec![format!("{}", link)]);
    }
    Content::Table(table)
}
fn create_mapq_hist(path: &Path, json: &MapJson) -> Result<(), Box<dyn std::error::Error>> {
    let hist_mapq = json.get_mapq_hist();
    let max = *hist_mapq.iter().max().expect("MapQ histogram empty") as f64;
    let len = hist_mapq.len() as f64;
    let root = BitMapBackend::new(path, (1024, 640)).into_drawing_area();
    root.fill(&WHITE)?;

    debug!("Making MapQ hist: {}", path.display());

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(35)
        .y_label_area_size(60)
        .margin(5)
        .caption("MapQ Histogram", ("sans-serif", 22.0).into_font())
        .build_cartesian_2d(-0.5..len + 0.5, 0.0..max)?;

    chart
        .configure_mesh()
        .light_line_style(&WHITE.mix(0.3))
        .y_desc("Fragments")
        .x_desc("MapQ")
        .y_label_formatter(&|y| format!("{:e}", y))
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;

    chart.draw_series(hist_mapq.iter().enumerate().map(|(x, y)| {
        Rectangle::new(
            [((x as f64) - 0.5, 0.0), ((x as f64) + 0.5, (*y as f64))],
            BLUE.mix(0.4).filled(),
        )
    }))?;
    debug!("Finished MapQ hist: {}", path.display());
    Ok(())
}

fn create_isize_hist(path: &Path, paired: &Paired) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Making isize hist: {}", path.display());
    let tlen = paired.template_len();
    // Get the bottom 99% of read lengths
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
    let thresh = (total as f64) * 0.99;
    let mut t = None;
    for (ix, y) in &tl {
        tmp += y;
        if *y > max {
            max = *y
        }
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
        .caption("Insert Size Distribution", ("sans-serif", 22.0).into_font())
        .build_cartesian_2d(0..*lim, 0..max)?;

    chart
        .configure_mesh()
        .light_line_style(&WHITE.mix(0.3))
        .y_desc("Fragments")
        .x_desc("Insert Size (bp)")
        .y_label_formatter(&|y| format!("{:e}", *y as f64))
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;

    chart.draw_series(LineSeries::new(
        tl.iter().map(|(x, y)| (*x, *y)),
        Into::<ShapeStyle>::into(&RED).stroke_width(3),
    ))?;
    debug!("Finished isize hist: {}", path.display());
    Ok(())
}
fn create_sample_body(
    project: &str,
    bc: &str,
    ds: &[&str],
    mapq_threshold: usize,
    dir: &Path,
    json: &MapJson,
    sample_report: bool,
) -> Result<HtmlElement, String> {
    let name = if sample_report { bc } else { ds[0] };
    let mut img_dir = dir.to_owned();
    img_dir.push("images");
    let mut mapq_hist_png = img_dir.clone();
    mapq_hist_png.push(format!("{}_mapq.png", name).as_str());
    let mut isize_hist_png = None;
    create_mapq_hist(&mapq_hist_png, json).map_err(|e| format!("{}", e))?;
    let mut body = HtmlElement::new("BODY", None, true);
    let mut path = HtmlElement::new("P", Some("id=\"path\""), true);
    if sample_report {
        path.push_string(format!("/{}/{}", project, bc));
    } else {
        path.push_string(format!("/{}/{}/{}", project, bc, name));
    }
    body.push(Content::Element(path));
    let mut back = HtmlElement::new("B", None, true);
    back.push_str("BACK");
    let t = if sample_report { "../index" } else { bc };
    let mut back_link = HtmlElement::new(
        "a",
        Some(format!("class=\"link\" href=\"{}.html\"", t).as_str()),
        true,
    );
    back_link.push_element(back);
    body.push_element(back_link);
    body.push_element(HtmlElement::new("BR", None, false));
    if sample_report {
        body.push_element(make_title(format!("SAMPLE {}", bc)));
    } else {
        body.push_element(make_title(format!("SAMPLE {} LANE {}", bc, name)));
    }
    body.push_element(make_section("Mapping Stats (Reads)"));
    body.push(make_reads_table(json));
    body.push_element(HtmlElement::new("BR><BR><BR", None, false));
    body.push_element(make_section(
        format!("Uniquely Mapping Fragments (MAPQ >= {})", mapq_threshold).as_str(),
    ));
    body.push(make_unique_table(mapq_threshold, json));
    body.push_element(HtmlElement::new("BR><BR><BR", None, false));
    body.push_element(make_section("Mapping Stats (Reads)"));
    body.push(make_bases_table(json));
    body.push_element(HtmlElement::new("BR><BR><BR", None, false));
    body.push_element(make_section("Bisulfite Conversion Rate"));
    body.push(make_conversion_table(json));
    let mut tp = img_dir;
    tp.push(format!("{}_isize.png", name).as_str());
    match json {
        MapJson::Paired(x) | MapJson::Unknown(x) => {
            body.push_element(HtmlElement::new("BR><BR><BR", None, false));
            body.push_element(make_section("Correct Pairs"));
            body.push(make_correct_pairs_table(x));
            create_isize_hist(&tp, x).map_err(|e| format!("{}", e))?;
            isize_hist_png = Some(tp);
        }
        _ => {
            fs::File::create(&tp).map_err(|e| format!("{}", e))?;
        }
    }
    body.push_element(HtmlElement::new("BR><BR><BR", None, false));
    body.push_element(make_section("Mapping Quality"));
    body.push(make_mapq_table(&mapq_hist_png));
    body.push_element(HtmlElement::new("BR><BR><BR", None, false));
    body.push_element(make_section("Read Lengths"));
    body.push(make_read_length_table(json)?);
    body.push_element(HtmlElement::new("BR><BR><BR", None, false));
    body.push_element(make_section("Mismatch Distribution"));
    body.push(make_mismatch_table(json)?);
    if let Some(x) = isize_hist_png {
        body.push_element(HtmlElement::new("BR><BR><BR", None, false));
        body.push_element(make_section("Insert Size"));
        body.push(make_isize_table(&x));
    }
    if sample_report && ds.len() > 1 {
        body.push_element(HtmlElement::new("BR><BR><BR", None, false));
        body.push_element(make_section("Mapping Lanes Reports"));
        body.push(make_links_table(ds));
    }
    Ok(body)
}

fn create_sample_html(
    project: &str,
    bc: &str,
    ds: &[&str],
    mapq_threshold: usize,
    dir: &Path,
    json: &MapJson,
    sample_report: bool,
) -> Result<(), String> {
    let l = ds.len();
    if l == 0 {
        return Err("No datasets supplied for map report".to_string());
    } else if l > 1 && !sample_report {
        return Err("Multiple datasets supplied for dataset map report".to_string());
    }
    let mut path = dir.to_owned();
    let name = if sample_report { bc } else { ds[0] };
    path.push(format!("{}.html", name).as_str());
    let mut html = HtmlPage::new(&path)?;
    let mut head_element = HtmlElement::new("HEAD", None, true);
    let mut style_element = HtmlElement::new("STYLE", Some("TYPE=\"text/css\""), true);
    style_element.push_str("<!--\n@import url(\"../../css/style.css\");\n-->");
    head_element.push_element(style_element);
    html.push_element(head_element);
    html.push_element(create_sample_body(
        project,
        bc,
        ds,
        mapq_threshold,
        dir,
        json,
        sample_report,
    )?);
    Ok(())
}

fn get_sample_sum(bc: &str, mapq_threshold: usize, mjson: &MapJson) -> SampleSummary {
    let barcode = bc.to_owned();
    let reads = match mjson {
        MapJson::Paired(x) | MapJson::Unknown(x) => {
            let rds = x.reads();
            let total = rds.get_total();
            total[0] + total[1]
        }
        MapJson::Single(x) => {
            let rds = x.reads();
            let total = rds.get_total();
            total[0]
        }
    };
    let (unique, fragments) = mjson.get_unique(mapq_threshold);
    let (ct1, ct2) = mjson.get_conversion_counts();
    let conversion = call::calc_conversion(&ct1);
    let overconversion = call::calc_conversion(&ct2);
    SampleSummary {
        barcode,
        reads,
        fragments,
        unique,
        conversion,
        overconversion,
    }
}

fn read_map_json(json_path: &Path) -> Result<MapJson, String> {
    let file = fs::File::open(json_path)
        .map_err(|e| format!("Couldn't open {}: {}", json_path.to_string_lossy(), e))?;
    let reader = Box::new(BufReader::new(file));
    Ok(MapJson::from_reader(reader).map_err(|e| {
        format!(
            "Couldn't parse JSON file {}: {}",
            json_path.to_string_lossy(),
            e
        )
    })?)
}

fn make_latex_sec(
    bc: &str,
    ds: Option<&str>,
    mapq_threshold: usize,
    json: &MapJson,
    sample_report: bool,
) -> Result<LatexSection, String> {
    let mut img_dir = PathBuf::from_str(bc).expect("Couldn't get Path from barcode");
    img_dir.push("images");
    let (name, stype, desc) = if sample_report {
        (bc, "section", "Sample")
    } else {
        (
            ds.expect("No dataset name supplied"),
            "subsection",
            "Dataset",
        )
    };
    let mut stype1 = "sub".to_string();
    stype1.push_str(stype);
    let mut sec = LatexSection::new(name);
    if sample_report {
        sec.push_str("\\newpage");
    }
    sec.push_string(format!(
        "\\{} {{{} {}}}",
        stype,
        desc,
        latex_escape_str(name)
    ));
    sec.push_string(format!("\\{}{{Mapping Stats (Reads)}}", stype1));
    sec.push(make_reads_latex_tab(json));
    sec.push_string(format!("\\{}{{Uniqueness}}", stype1));
    sec.push(make_unique_latex_tab(mapq_threshold, json));
    sec.push_string(format!("\\{}{{Mapping Stats (Bases)}}", stype1));
    sec.push(make_bases_latex_tab(json));
    sec.push_string(format!("\\{}{{Bisulfite Conversion Rate}}", stype1));
    sec.push(make_conversion_latex_tab(json));
    sec.push_string(format!("\\{}{{Mapping Quality Histogram}}", stype1));
    sec.push_string(format!(
        "\\includegraphics[width=12cm]{{{}}}",
        img_dir.join(format!("{}_mapq", name).as_str()).display()
    ));
    sec.push_string(format!("\\{}{{Read Lengths}}", stype1));
    sec.push(make_read_length_latex_tab(json)?);
    sec.push_string(format!("\\{}{{Mismatch distribution}}", stype1));
    sec.push(make_mismatch_length_latex_tab(json)?);
    if json.get_type() != MapJsonType::Single {
        sec.push_string(format!("\\{}{{Insert Size}}", stype1));
        sec.push_string(format!(
            "\\includegraphics[width=12cm]{{{}}}",
            img_dir.join(format!("{}_isize", name).as_str()).display()
        ));
    }
    Ok(sec)
}

fn create_sample_report(job: ReportJob) -> Result<(), String> {
    match job.job {
        RepJob::Sample(v) => {
            info!(
                "Create sample mapping report for {}/{}",
                job.project, job.barcode
            );
            let mut dataset_secs = if v.datasets.len() > 1 {
                Some(SectionArray::new())
            } else {
                None
            };
            let mut mrg_json: Option<MapJson> = None;
            let mut dsets: Vec<&str> = Vec::new();
            for (ds, json_path) in v.datasets.iter() {
                let json = read_map_json(&json_path)?;
                if let Some(ref mut sa) = dataset_secs {
                    let ds_sec =
                        make_latex_sec(&job.barcode, Some(ds), v.mapq_threshold, &json, false)?;
                    sa.push(ds_sec);
                }
                mrg_json = match mrg_json {
                    Some(j) => Some(j.merge(json)),
                    None => Some(json),
                };
                dsets.push(ds);
            }
            dsets.sort_unstable();
            match mrg_json {
                Some(mjson) => {
                    let sample_sum = get_sample_sum(&job.barcode, v.mapq_threshold, &mjson);
                    if let Ok(mut sum_vec) = v.summary.lock() {
                        sum_vec.push(sample_sum);
                    } else {
                        return Err("Couldn't obtain lock on sample summary".to_string());
                    }
                    let mut latex_sec =
                        make_latex_sec(&job.barcode, None, v.mapq_threshold, &mjson, true)?;
                    if let Some(sa) = dataset_secs {
                        latex_sec.push(LatexContent::SecArray(sa));
                    }
                    if let Ok(mut ldoc) = v.latex_doc.lock() {
                        ldoc.push_section(latex_sec)?;
                    } else {
                        return Err("Couldn't obtain lock on latex doc".to_string());
                    }
                    create_sample_html(
                        &job.project,
                        &job.barcode,
                        &dsets,
                        v.mapq_threshold,
                        &job.bc_dir,
                        &mjson,
                        true,
                    )
                }
                None => Err(format!("No merged JSON structure for {}", &job.barcode)),
            }
        }
        RepJob::Dataset(v) => {
            let json = read_map_json(&v.json_path)?;
            info!(
                "Create dataset mapping report for {}/{}/{}",
                job.project, job.barcode, v.dataset
            );
            create_sample_html(
                &job.project,
                &job.barcode,
                &[&v.dataset],
                v.mapq_threshold,
                &job.bc_dir,
                &json,
                false,
            )
        }
        _ => Err("Invalid command".to_string()),
    }
}

fn create_summary(
    dir: &Path,
    summary: Arc<Mutex<Vec<SampleSummary>>>,
    latex_doc: Arc<Mutex<LatexBare>>,
) -> Result<(), String> {
    info!("Create mapping summary report");
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
    let mut ltable = LatexTable::new();
    table.add_header(vec![
        "Sample",
        "Reads",
        "Fragments",
        "Unique (%)",
        "Conv. Rate",
        "Over Conv. Rate",
    ]);
    ltable.add_header(vec![
        "Sample",
        "Reads",
        "Fragments",
        "Unique (%)",
        "Conv. Rate",
        "Over Conv. Rate",
    ]);
    if let Ok(mut sum_vec) = summary.lock() {
        sum_vec.sort_by(|a, b| a.barcode.cmp(&b.barcode));
        for s in sum_vec.iter() {
            let mut row = Vec::new();
            let mut lrow = Vec::new();
            let mut link = HtmlElement::new(
                "a",
                Some(format!("class=\"link\" href=\"{}/{}.html\"", s.barcode, s.barcode).as_str()),
                true,
            );
            link.push_str(s.barcode.as_str());
            row.push(format!("&#187 {}", link));
            lrow.push(s.barcode.clone());
            row.push(format!("{}", s.reads));
            lrow.push(format!("{}", s.reads));
            row.push(format!("{}", s.fragments));
            lrow.push(format!("{}", s.fragments));
            row.push(format!(
                "{} ({:.2} %)",
                s.unique,
                pct(s.unique, s.fragments)
            ));
            lrow.push(format!(
                "{} ({:.2} %)",
                s.unique,
                pct(s.unique, s.fragments)
            ));
            let conv = if let Some(x) = s.conversion {
                format!("{:.4}", x)
            } else {
                "NA".to_string()
            };
            row.push(conv.clone());
            lrow.push(conv);
            let conv = if let Some(x) = s.overconversion {
                format!("{:.4}", x)
            } else {
                "NA".to_string()
            };
            row.push(conv.clone());
            lrow.push(conv);
            table.add_row(row);
            ltable.add_row(lrow);
        }
    } else {
        return Err("Couldn't obtain lock on sample summary".to_string());
    }
    body.push(Content::Table(table));
    html.push_element(body);
    if let Ok(mut ldoc) = latex_doc.lock() {
        ldoc.push(LatexContent::Text(
            "\\section{{Sample Summary Statistics}}".to_string(),
        ));
        ldoc.push(LatexContent::Table(ltable));
        Ok(())
    } else {
        Err("Couldn't obtain lock on latex doc".to_string())
    }
}

fn worker_thread(
    tx: mpsc::Sender<(isize, usize)>,
    rx: mpsc::Receiver<Option<ReportJob>>,
    idx: isize,
) {
    loop {
        match rx.recv() {
            Ok(Some(job)) => {
                let job_ix = job.ix;
                if let Err(e) = create_sample_report(job) {
                    error!("Error creating sample report: {}", e);
                    tx.send((-(idx + 1), job_ix))
                        .expect("Error sending message to parent");
                } else {
                    tx.send((idx, job_ix))
                        .expect("Error sending message to parent");
                }
            }
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
}

fn prepare_jobs(
    svec: &[SampleJsonFiles],
    project: &str,
    mapq_threshold: usize,
    summary: Arc<Mutex<Vec<SampleSummary>>>,
    latex_doc: Arc<Mutex<LatexBare>>,
) -> Vec<ReportJob> {
    let mut v = Vec::new();
    for hr in svec.iter() {
        // First push sample report job
        let mut sjob = SampleJob::new(summary.clone(), latex_doc.clone(), mapq_threshold);
        let l = hr.json_files.len();
        for (ds, path) in hr.json_files.iter() {
            if l > 1 {
                sjob.depend.push(v.len());
                let djob = DatasetJob::new(ds, path, mapq_threshold);
                v.push(ReportJob::new(
                    &hr.barcode,
                    project,
                    &hr.bc_dir,
                    RepJob::Dataset(djob),
                ));
            }
            sjob.add_dataset(ds, path);
        }
        let sample_job = ReportJob::new(&hr.barcode, project, &hr.bc_dir, RepJob::Sample(sjob));
        v.push(sample_job);
    }
    for (ix, job) in v.iter_mut().enumerate() {
        job.ix = ix
    }
    v
}

pub fn copy_css(output_dir: &Path, css: &Path) -> Result<(), String> {
    let t = output_dir.parent().unwrap_or_else(|| Path::new("."));
    let css_dir = t.join("css");
    // Output directories are normally automatically generated by the scheduler, but in this case the css_dir
    // by map_report or call_report and so the normal system fails and we will do it manually
    fs::create_dir_all(&css_dir).expect("Could not create required output directory for css file");
    let out_css = css_dir.join("style.css");
    fs::copy(css, out_css).map_err(|e| format!("Error copying css file: {}", e))?;
    Ok(())
}

pub fn make_map_report(
    sig: Arc<AtomicUsize>,
    outputs: &[PathBuf],
    project: Option<String>,
    css: &Path,
    mapq_threshold: usize,
    n_cores: usize,
    svec: Vec<SampleJsonFiles>,
) -> Result<(), String> {
    utils::check_signal(Arc::clone(&sig))?;
    info!("Making mapping report");
    let project = project.unwrap_or_else(|| "gemBS".to_string());
    let report_tex_path = outputs.first().expect("No output files for map report");
    let output_dir = report_tex_path
        .parent()
        .expect("No parent directory found for map report");
    // Set up worker threads
    // Maximum parallel jobs that we could do if there were enough cores is the nmber of datasets
    let n_dsets = svec.iter().fold(0, |sum, x| sum + x.json_files.len());
    let n_workers = if n_cores > n_dsets { n_dsets } else { n_cores };
    let (ctr_tx, ctr_rx) = mpsc::channel();
    let mut avail = Vec::new();
    let mut workers = Vec::new();
    let mut jobs = Vec::new();
    for ix in 0..n_workers {
        let (tx, rx) = mpsc::channel();
        let ctr = mpsc::Sender::clone(&ctr_tx);
        let handle = thread::spawn(move || worker_thread(ctr, rx, ix as isize));
        workers.push(Worker { handle, tx, ix });
        avail.push(ix);
    }
    // Prepare jobs
    let summary = Arc::new(Mutex::new(Vec::new()));
    let latex_doc = Arc::new(Mutex::new(LatexBare::new(&report_tex_path)?));

    let mut job_vec = prepare_jobs(
        &svec,
        &project,
        mapq_threshold,
        summary.clone(),
        latex_doc.clone(),
    );
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
                        RepJob::Dataset(_) => {
                            x = Some(ix);
                            waiting = false;
                            break;
                        }
                        RepJob::Sample(v) => {
                            let mut ready = true;
                            for i in v.depend.iter() {
                                if job_vec[*i].status != JobStatus::Completed {
                                    ready = false;
                                    break;
                                }
                            }
                            if ready {
                                x = Some(ix);
                                waiting = false;
                                break;
                            } else {
                                waiting = true;
                            }
                        }
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
            debug!("Sending map report job to worker {}", idx);
            workers[idx]
                .tx
                .send(Some(job_vec[jix].clone()))
                .expect("Error sending new command to map report worker thread");
            match ctr_rx.try_recv() {
                Ok((x, ix)) if x >= 0 => {
                    debug!("Job completion by map worker thread {}", x);
                    jobs.retain(|ix| *ix != x);
                    avail.push(x as usize);
                    job_vec[ix].status = JobStatus::Completed;
                }
                Ok((x, _)) => {
                    error!("Error received from worker thread {}", -(x + 1));
                    abort = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(e) => {
                    error!("Scheduler thread received error: {}", e);
                    abort = true;
                    break;
                }
            }
        } else {
            if let Some(idx) = worker_ix {
                avail.push(idx);
            }
            if !jobs.is_empty() {
                match ctr_rx.recv_timeout(time::Duration::from_millis(1000)) {
                    Ok((x, ix)) if x >= 0 => {
                        debug!("Job completion by worker thread {}", x);
                        jobs.retain(|ix| *ix != x);
                        avail.push(x as usize);
                        job_vec[ix].status = JobStatus::Completed;
                    }
                    Ok((x, _)) => {
                        error!("Error received from worker thread {}", -(x + 1));
                        abort = true;
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(e) => {
                        error!("Scheduler thread received error: {}", e);
                        abort = true;
                        break;
                    }
                }
            } else if waiting {
                thread::sleep(time::Duration::from_secs(1))
            } else {
                break;
            }
        }
    }
    if !abort {
        for w in workers.drain(..) {
            if w.tx.send(None).is_err() {
                debug!(
                    "Error when trying to send shutdown signal to worker thread {}",
                    w.ix
                );
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
    if abort {
        Err("Map-report generation failed".to_string())
    } else {
        create_summary(output_dir, summary, latex_doc)?;
        copy_css(output_dir, css)?;
        Ok(())
    }
}

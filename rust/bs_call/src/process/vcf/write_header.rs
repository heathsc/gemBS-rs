use std::io;
use std::collections::HashSet;

use chrono::prelude::*;

use crate::config::*;
use crate::defs::contigs;
use crate::htslib::{VcfHeader, SamFile};

const FIXED_HEADERS: [&str; 20] = [
	"##INFO=<ID=CX,Number=1,Type=String,Description=\"5 base sequence context (from position -2 to +2 on the positive strand) determined from the reference\">",
	"##FILTER=<ID=fail,Description=\"No sample passed filters\">",
	"##FILTER=<ID=q20,Description=\"Genotype Quality below 20\">",
	"##FILTER=<ID=qd2,Description=\"Quality By Depth below 2\">",
	"##FILTER=<ID=fs60,Description=\"Fisher Strand above 60\">",
	"##FILTER=<ID=mq40,Description=\"RMS Mapping Quality below 40\">",
	"##FILTER=<ID=mac1,Description=\"Minor allele count <= 1\">",
	"##FORMAT=<ID=GT,Number=1,Type=String,Description=\"Genotype\">",
	"##FORMAT=<ID=FT,Number=1,Type=String,Description=\"Sample Genotype Filter\">",
	"##FORMAT=<ID=GL,Number=G,Type=Float,Description=\"Genotype Likelihood\">",
	"##FORMAT=<ID=GQ,Number=1,Type=Integer,Description=\"Phred scaled conditional genotype quality\">",
	"##FORMAT=<ID=DP,Number=1,Type=Integer,Description=\"Read Depth (non converted reads only)\">",
	"##FORMAT=<ID=MQ,Number=1,Type=Integer,Description=\"RMS Mapping Quality\">",
	"##FORMAT=<ID=QD,Number=1,Type=Integer,Description=\"Quality By Depth (Variant quality / read depth (non-converted reads only))\">",
	"##FORMAT=<ID=MC8,Number=8,Type=Integer,Description=\"Base counts: non-informative for methylation (ACGT) followed by informative for methylation (ACGT)\">",
	"##FORMAT=<ID=AMQ,Number=.,Type=Integer,Description=\"Average base quailty for where MC8 base count non-zero\">",
	"##FORMAT=<ID=CS,Number=1,Type=String,Description=\"Strand of Cytosine relative to reference sequence (+/-/+-/NA)\">",
	"##FORMAT=<ID=CG,Number=1,Type=String,Description=\"CpG Status (from genotype calls: Y/N/H/?)\">",
	"##FORMAT=<ID=CX,Number=1,Type=String,Description=\"5 base sequence context (from position -2 to +2 on the positive strand) determined from genotype call\">",
	"##FORMAT=<ID=FS,Number=1,Type=Integer,Description=\"Phred scaled log p-value from Fishers exact test of strand bias\"",
];

fn find_tags<'a>(s: &'a str, tags: &[&str]) -> Vec<Option<&'a str>> {
	let n = tags.len();
	let mut tg = vec![None; n];
	
	for fd in s.split('\t') {
		for (ix, t) in tags.iter().enumerate() {
			if fd.starts_with(t) && fd[2..3].eq(":") && fd.len() > 3  { tg[ix] = Some(&fd[3..]) }
		}
	}
	tg
}

fn add_sample_info<'a>(hd: &mut VcfHeader, text: &'a str, bench: bool) -> io::Result<Option<&'a str>> {
	let mut bc_set = HashSet::new();
	for s in text.lines() {
		if s.starts_with("@RG\t") {
			let tags = find_tags(s, &["BC", "SM", "DS"]);
			if let Some(bc) = tags[0] {
				if !bc_set.insert(bc) && !bench {
					let mut sbuf = format!("##bs_call_sample_info=<ID=\"{}\"", bc);
					if let Some(sm) = tags[1] { sbuf.push_str(format!(",SM=\"{}\"", sm).as_str()); }
					if let Some(ds) = tags[2] { sbuf.push_str(format!(",DS=\"{}\"", ds).as_str()); }
					sbuf.push('>');
					hd.append(&sbuf)?;
				}
			}
		}
	}
	if let Some(bc) = bc_set.iter().next() {
		Ok(Some(bc))
	} else { Ok(None) }
}

fn add_seq_info(hd: &mut VcfHeader, ctgs: &[contigs::CtgInfo], sam_file: &SamFile) -> io::Result<()> {
	for s in sam_file.text().lines() {
		if s.starts_with("@SQ\t") {
			let tags = find_tags(s, &["SN", "AS", "M5", "SP"]);
			if let Some(sn) = tags[0] {
				let tid = sam_file.name2tid(sn).expect("COuldn't get tid for SAM Sequence");
				if ctgs[tid].in_header() {
					let ln = sam_file.tid2len(tid);
					let mut sbuf = format!("##contig=<ID={},length={}", sn, ln);
					if let Some(x) = tags[1] { sbuf.push_str(format!(",assembly={}", x).as_str()); }
					if let Some(x) = tags[2] { sbuf.push_str(format!(",md5={}", x).as_str()); }
					if let Some(x) = tags[3] { sbuf.push_str(format!(",sp={}", x).as_str()); }
					sbuf.push('>');
					hd.append(&sbuf)?;
				}
			}
		}
	}
	Ok(())
}

pub fn write_vcf_header(bs_cfg: &mut BsCallConfig, version: &str) -> io::Result<()> {
	let mut hd = &mut bs_cfg.vcf_output.hdr;
	let sam_file = &bs_cfg.sam_input;
	let chash = &bs_cfg.conf_hash;
	let mut sbuf = format!("##fileformat={}", hd.get_version());
	hd.append(&sbuf)?;
	let benchmark = chash.get_bool("benchmark_mode");
	if !benchmark {
		sbuf = format!("##fileDate(dd/mm/yyyy)={}", Local::now().format("%d/%m/%Y"));
		hd.append(&sbuf)?;
		sbuf = format!("##source={},under_conversion={},over_conversion={},mapq_thresh={},bq_thresh={}", version,
			chash.get_float("under_conversion"), chash.get_float("over_conversion"),
			chash.get_int("mapq_threshold"), chash.get_int("bq_threshold"));
		hd.append(&sbuf)?;
        // TODO - should add dbSNP header info here
	}
	let sam_sample = add_sample_info(&mut hd, sam_file.text(), benchmark)?;
	let contigs = &mut bs_cfg.contigs;
	add_seq_info(&mut hd, contigs, sam_file)?;	
	for line in FIXED_HEADERS.iter() { hd.append(line)?; }
	let sample = if let Some(s) = chash.get_str("sample") { s }
	else if let Some(s) = sam_sample { s }
	else { "SAMPLE" };
	hd.add_sample(sample)?;
	hd.sync()?;
	// Get VCF/BCF header IDs for contigs and filters
	contigs::set_contig_vcf_ids(&hd, contigs, sam_file); 
	// And write out header
	bs_cfg.vcf_output.write_hdr()?;	
	Ok(())
}

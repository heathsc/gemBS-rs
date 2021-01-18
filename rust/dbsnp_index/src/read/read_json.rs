use std::str::FromStr;
use std::collections::HashMap;
use lazy_static::lazy_static;

use json_rs::*;

use super::*;
use crate::snp::SnpBuilder;

enum JsKey {
	RefsnpId, PrimarySnapshotData, PlacementsWithAllele, IsPtlp, Alleles,
	Allele, Spdi, InsertedSequence, DeletedSequence, Position, SeqId,
	AlleleAnnotations, Frequency, StudyName, AlleleCount, TotalCount, Observation
}

const IN_PRIMARY_SNAPSHOT_DATA: u32 = 1;
const IN_PLACEMENTS_WITH_ALLELE: u32 = 2;
const IN_ALLELES: u32 = 4;
const IN_ALLELE: u32 = 8;
const IN_SPDI: u32 = 0x10;
const IN_ALLELE_ANNOTATIONS: u32 = 0x20;
const IN_FREQUENCY: u32 = 0x40;
const IN_OBSERVATION:u32 = 0x80;
const IS_PTLP: u32 = 0x100;
// const HAS_POSITION:u32 = 0x200;
const FREQ_ALLELES_OK: u32 = 0x400;
const VALID_SNP: u32 = 0x800;
const SEEN_FREQ_INSERTED_SEQUENCE: u32 = 0x1000;
const SEEN_FREQ_DELETED_SEQUENCE: u32 = 0x2000;
const SEEN_ALLELE_COUNT: u32 = 0x4000;
const SEEN_TOTAL_COUNT: u32 = 0x8000;
const STUDY_NAME_OK: u32 = 0x10000;

const FREQ_ALLELE_FLAGS: u32 = SEEN_FREQ_INSERTED_SEQUENCE | SEEN_FREQ_DELETED_SEQUENCE;
const FREQ_FLAGS: u32 = FREQ_ALLELES_OK | SEEN_ALLELE_COUNT | SEEN_TOTAL_COUNT | STUDY_NAME_OK;

lazy_static! {
    static ref JS_KEYS: HashMap<&'static str, JsKey> = {
        let mut m = HashMap::new();
		m.insert("refsnp_id", JsKey::RefsnpId);
		m.insert("primary_snapshot_data", JsKey::PrimarySnapshotData);
		m.insert("placements_with_allele", JsKey::PlacementsWithAllele);
		m.insert("is_ptlp", JsKey::IsPtlp);
		m.insert("alleles", JsKey::Alleles);
		m.insert("allele", JsKey::Allele);
		m.insert("spdi", JsKey::Spdi);
		m.insert("inserted_sequence", JsKey::InsertedSequence);
		m.insert("deleted_sequence", JsKey::DeletedSequence);
		m.insert("position", JsKey::Position);
		m.insert("seq_id", JsKey::SeqId);
		m.insert("allele_annotations", JsKey::AlleleAnnotations);
		m.insert("frequency", JsKey::Frequency);
		m.insert("study_name", JsKey::StudyName);
		m.insert("allele_count", JsKey::AlleleCount);
		m.insert("total_count", JsKey::TotalCount);
		m.insert("observation", JsKey::Observation);
        m
    };
}

#[derive(Debug, Default)]
struct JsonSnp<'a> {
	name: Option<&'a str>,
	cname: Option<&'a str>,
	pos: Option<u32>,
	maf: Option<f32>,
	allele_count: u32,
	total_count: u32,
	a: u32,
	b: u32,
	mask: u32,
	inserted_sequence: Option<u8>,
	deleted_sequence: Option<u8>,
	alleles: [u8; 2],	
}

fn handle_json_tokens<'a>(jtxt: &'a str, jtok: &[JTok], jsnp: &mut JsonSnp<'a>, level: usize) -> usize {
	if jtok.is_empty() || (jsnp.mask & VALID_SNP) != 0 { 0 }
	else { 
		match jtok[0].tok_type {
			JType::String | JType::Number | JType::Bool(_) => 1,
			JType::Object => {
				let mut j = 0;
				for _ in 0 .. jtok[0].size {
					let ntok = &jtok[j + 1];
					assert_eq!(ntok.tok_type, JType::String);
					let mut child_processed = false;
					if ntok.size > 0 {
						if let Some(key) = JS_KEYS.get(&jtxt[ntok.start..ntok.end]) {
							let ntok1 = &jtok[j + 2];
							match key {
								JsKey::RefsnpId => {
									if level == 0 && ntok1.tok_type == JType::String {
										jsnp.name = Some(&jtxt[ntok1.start..ntok1.end]);
										j += 2;
										child_processed = true;
									}
								},
								JsKey::PrimarySnapshotData => {
									if level == 0 && ntok1.tok_type == JType::Object {
										jsnp.mask |= IN_PRIMARY_SNAPSHOT_DATA;
										j += 1 + handle_json_tokens(jtxt, &jtok[j + 2..], jsnp, level + 1);
										jsnp.mask &= !IN_PRIMARY_SNAPSHOT_DATA;
										if let (Some(s1), Some(s2)) = (jsnp.inserted_sequence, jsnp.deleted_sequence) {
											if s1 != s2 && jsnp.pos.is_some() {
												jsnp.mask |= VALID_SNP;
												if jsnp.total_count > 0 {
													let z = (jsnp.allele_count as f32) / (jsnp.total_count as f32);
													jsnp.maf = Some(if z > 0.5 { 1.0 - z } else { z })
												}
											}
										}
										child_processed = true;
									}
									
								},
								JsKey::PlacementsWithAllele => {
									if level == 1 && ntok1.tok_type == JType::Array && (jsnp.mask & IN_PRIMARY_SNAPSHOT_DATA) != 0 {
										jsnp.mask |= IN_PLACEMENTS_WITH_ALLELE;
										j += 1 + handle_json_tokens(jtxt, &jtok[j + 2..], jsnp, level + 1);
										jsnp.mask &= !IN_PLACEMENTS_WITH_ALLELE;
										child_processed = true;
									}
								},
								JsKey::AlleleAnnotations => {
									if level == 1 && ntok1.tok_type == JType::Array && (jsnp.mask & IN_PRIMARY_SNAPSHOT_DATA) != 0 {
										jsnp.mask |= IN_ALLELE_ANNOTATIONS;
										j += 1 + handle_json_tokens(jtxt, &jtok[j + 2..], jsnp, level + 1);
										jsnp.mask &= !IN_ALLELE_ANNOTATIONS;
										child_processed = true;
									}
								},
								JsKey::IsPtlp => {
									if level == 3 && (jsnp.mask & IN_PLACEMENTS_WITH_ALLELE) != 0 {
										if let JType::Bool(x) = ntok1.tok_type {
											if x { jsnp.mask |= IS_PTLP } else { jsnp.mask &= !IS_PTLP }	
											child_processed = true;
											j += 2;
										}
									}
								},
								JsKey::Frequency => {
									if level == 3 && ntok1.tok_type == JType::Array && (jsnp.mask & IN_ALLELE_ANNOTATIONS) != 0 {
										jsnp.mask = (jsnp.mask & !FREQ_FLAGS) | IN_FREQUENCY;
										j += 2;
										for _ in 0 .. ntok1.size {
											j += handle_json_tokens(jtxt, &jtok[j + 1..], jsnp, level + 2);
											if (jsnp.mask & FREQ_FLAGS) == FREQ_FLAGS && jsnp.a <= jsnp.b {
												jsnp.allele_count += jsnp.a;
												jsnp.total_count += jsnp.b;
											}
											jsnp.mask &= !FREQ_FLAGS;
										}
										jsnp.mask &= !IN_FREQUENCY;
										child_processed = true;
									}
								},
								JsKey::Alleles => {
									if level == 3 && ntok1.tok_type == JType::Array && 
										(jsnp.mask & (IN_PLACEMENTS_WITH_ALLELE | IS_PTLP)) == (IN_PLACEMENTS_WITH_ALLELE | IS_PTLP) {
										jsnp.mask |= IN_ALLELES;
										j += 1 + handle_json_tokens(jtxt, &jtok[j + 2..], jsnp, level + 1);
										jsnp.mask &= !IN_ALLELES;
										child_processed = true;
									}									
								},
								JsKey::Allele => {
									if level == 5 && ntok1.tok_type == JType::Object && (jsnp.mask & IN_ALLELES) != 0 {
										jsnp.mask |= IN_ALLELE;
										jsnp.inserted_sequence = None;
										jsnp.deleted_sequence = None;
										let old_pos = jsnp.pos;
										j += 1 + handle_json_tokens(jtxt, &jtok[j + 2..], jsnp, level + 1);
										if let (Some(s1), Some(s2)) = (jsnp.inserted_sequence, jsnp.deleted_sequence) {
											if s1 != s2 && jsnp.pos.is_some() {
												jsnp.alleles = [s1, s2];
											} else {
												jsnp.pos = old_pos;
											}
										}
										jsnp.mask &= !IN_ALLELE;
										child_processed = true;
									}									
								},
								JsKey::Observation => {
									if level == 5 && ntok1.tok_type == JType::Object && (jsnp.mask & IN_FREQUENCY) != 0 {
										jsnp.mask = (jsnp.mask & !FREQ_ALLELE_FLAGS) | ( IN_OBSERVATION | FREQ_ALLELES_OK);
										j += 1 + handle_json_tokens(jtxt, &jtok[j + 2..], jsnp, level + 1);
										if (jsnp.mask & (FREQ_ALLELE_FLAGS | FREQ_ALLELES_OK)) != (FREQ_ALLELE_FLAGS | FREQ_ALLELES_OK) {
											jsnp.mask &= !(FREQ_ALLELE_FLAGS | FREQ_ALLELES_OK | IN_OBSERVATION)
										} else {
											jsnp.mask &= !(FREQ_ALLELE_FLAGS | IN_OBSERVATION);
										}
										child_processed = true;
									}									
								},
								JsKey::Spdi => {
									if level == 6 && ntok1.tok_type == JType::Object && (jsnp.mask & IN_ALLELE) != 0 {
										jsnp.mask |= IN_SPDI;
										j += 1 + handle_json_tokens(jtxt, &jtok[j + 2..], jsnp, level + 1);
										jsnp.mask &= !IN_SPDI;
										child_processed = true;
									}									
								},
								JsKey::Position => {
									if level == 7 && ntok1.tok_type == JType::Number && (jsnp.mask & IN_SPDI) != 0 {
										if let Ok(x) = <u32>::from_str(&jtxt[ntok1.start..=ntok1.end]) { jsnp.pos = Some(x) }
										j += 2;
										child_processed = true;
									}									
								},
								JsKey::SeqId => {
									if level == 7 && ntok1.tok_type == JType::String && (jsnp.mask & IN_SPDI) != 0 {
										jsnp.cname = Some(&jtxt[ntok1.start..ntok1.end]);
										j += 2;
										child_processed = true;
									}									
								},
								JsKey::InsertedSequence => {
									if level == 7 && ntok1.tok_type == JType::String && (jsnp.mask & IN_SPDI) != 0 {
										if ntok1.end - ntok1.start == 1 { jsnp.inserted_sequence = Some(jtxt[ntok1.start..ntok1.end].as_bytes()[0]) }
									} else if level == 6 && ntok1.tok_type == JType::String && (jsnp.mask & IN_OBSERVATION) != 0 {
										jsnp.mask |= SEEN_FREQ_INSERTED_SEQUENCE;
										if ntok1.end - ntok1.start != 1 || Some(jtxt[ntok1.start..ntok1.end].as_bytes()[0]) != jsnp.inserted_sequence {
											jsnp.mask &= !FREQ_ALLELES_OK;
										}
									}
									j += 2;
									child_processed = true;
								},
								JsKey::DeletedSequence => {
									if level == 7 && ntok1.tok_type == JType::String && (jsnp.mask & IN_SPDI) != 0 {
										if ntok1.end - ntok1.start == 1 { jsnp.deleted_sequence = Some(jtxt[ntok1.start..ntok1.end].as_bytes()[0]) }
									} else if level == 6 && ntok1.tok_type == JType::String && (jsnp.mask & IN_OBSERVATION) != 0 {
										jsnp.mask |= SEEN_FREQ_DELETED_SEQUENCE;
										if ntok1.end - ntok1.start != 1 || Some(jtxt[ntok1.start..ntok1.end].as_bytes()[0]) != jsnp.deleted_sequence {
											jsnp.mask &= !FREQ_ALLELES_OK;
										}
									}
									j += 2;
									child_processed = true;
								},
								JsKey::StudyName => {
									if level == 5 && ntok1.tok_type == JType::String && (jsnp.mask & IN_FREQUENCY) != 0 {
										jsnp.mask |= STUDY_NAME_OK;
										j += 2;
										child_processed = true;
									}									
								},
								JsKey::AlleleCount => {
									if level == 5 && ntok1.tok_type == JType::Number && (jsnp.mask & IN_FREQUENCY) != 0 {
										if let Ok(x) = <u32>::from_str(&jtxt[ntok1.start..=ntok1.end]) { 
											jsnp.a = x;
											jsnp.mask |= SEEN_ALLELE_COUNT; 
										}
										j += 2;
										child_processed = true;
									}									
								},
								JsKey::TotalCount => {
									if level == 5 && ntok1.tok_type == JType::Number && (jsnp.mask & IN_FREQUENCY) != 0 {
										if let Ok(x) = <u32>::from_str(&jtxt[ntok1.start..=ntok1.end]) { 
											jsnp.b = x;
											jsnp.mask |= SEEN_TOTAL_COUNT; 
										}
										j += 2;
										child_processed = true;
									}									
								},								
							}
						}		
					}
					if !child_processed {
						j += 1;
						if ntok.size > 0 { j += handle_json_tokens(jtxt, &jtok[j + 1..], jsnp, level + 1); }
					}
				}
				j + 1
			},
			JType::Array => {
				let mut j = 0;
				for _ in 0 .. jtok[0].size {
					if (jsnp.mask & VALID_SNP) != 0 { break }
					j += handle_json_tokens(jtxt, &jtok[j + 1..], jsnp, level + 1);
				}
				j + 1	
			},
			JType::Null => 0,
		}
	}
}

fn snp_from_json(s: &str, rb: &mut SnpBuilder) -> Option<Snp> {
	let mut jparse = JParse::new(s);
	trace!("Parsing JSON string");
	let r = jparse.parse();
	if r.is_ok() {
		let mut jsnp: JsonSnp = Default::default();
		handle_json_tokens(s, jparse.tokens(), &mut jsnp, 0);
		if (jsnp.mask & VALID_SNP) != 0 {
			if let (Some(name), Some(cname), Some(pos)) = (jsnp.name, jsnp.cname, jsnp.pos) {
//				println!("rs{} {} {} {:?}", name, cname, pos, jsnp.maf);
				rb.build_snp(name, cname, pos, jsnp.maf)
			} else { None }
		} else { None }
	} else { None }
}

pub fn process_json_line(buf: &str, builder: &mut SnpBuilder, rbuf: &mut ReaderBuf) {
	if let Some(snp) = snp_from_json(&buf, builder) { 
		rbuf.add_snp(snp) 
	}	
}

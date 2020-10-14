use std::{io, cmp};
use std::sync::mpsc;
use std::collections::HashMap;

use crate::htslib::*;
use crate::config::BsCallConfig;
use super::records::{ReadEnd, Map};
use crate::stats::{StatJob, FSBaseLevelType, FSReadLevelType, FSType};

enum ReadState {
	Duplicate,
	Present(usize),
} 

struct StateInner {
	tid: u32,
	start_x: u32,
	end_x: u32,
	curr_x: u32,
	idx: usize,	
}

struct State(Option<StateInner>);

impl State {
	fn from_map(map: &Map) -> StateInner {
		let map_pos = map.map_pos;
		let start_x = map_pos.pos;
		let end_x = start_x + map.cigar.rlen();
		StateInner{tid: map_pos.tid, start_x, end_x, curr_x: start_x, idx: 0 }
	}
	
	fn update(&mut self, map: &Map, idx: usize) -> StateChange {
		if let Some(mut state) = self.0.take() { 
			let mut change = StateChange::None;
			let mp = map.map_pos;
			let start_x = mp.pos;
			let end_x = start_x + map.cigar.rlen();
			let state_cp = |s: &mut StateInner| {
				s.start_x = start_x;
				s.curr_x = start_x;
				s.end_x = end_x;
				s.idx = 0;
			};
			if mp.tid != state.tid {
				change = StateChange::NewContig((state.tid, state.start_x, state.end_x));
				state.tid = mp.tid;
				state_cp(&mut state);
			} else if start_x > state.end_x {
				change = StateChange::NewBlock((state.start_x, state.end_x));
				state_cp(&mut state);
			} else {
				if start_x < state.curr_x { panic!("BAM is not sorted!") }
				if start_x > state.curr_x {
					state.curr_x = start_x;
					state.idx = idx;
				}
				state.end_x = cmp::max(state.end_x, end_x);
			}
			self.0 = Some(state);
			change
		} else { 
			self.0 = Some(State::from_map(map));
			StateChange::Init
		}
	} 
}

enum StateChange { 
	None,
	Init,
	NewBlock((u32, u32)),
	NewContig((u32, u32, u32)),
}

pub fn read_data(bs_cfg: &mut BsCallConfig, stat_tx: mpsc::Sender<StatJob>) -> io::Result<()> {
	bs_cfg.sam_input.set_region_itr(&bs_cfg.regions)?;
	let mut fs_stats = FSType::new();
	
	let hdr = &bs_cfg.sam_input.hdr;
	let keep_duplicates = bs_cfg.conf_hash.get_bool("keep_duplicates");
	let mut brec = BamRec::new().unwrap();
	let mut reads: Vec<Option<ReadEnd>> = Vec::new();
	let mut state_hash: HashMap<String, ReadState> = HashMap::new();
	let mut curr_state = State(None);
	loop {
		match bs_cfg.sam_input.get_next(&mut brec) {
			SamReadResult::Ok => (),
			SamReadResult::EOF => break,
			_ => panic!("Error reading record"),
		} 
		let (read_end, read_flag) = ReadEnd::from_bam_rec(&bs_cfg.conf_hash, hdr, &brec);
		if let Some(mut read) = read_end {
			let map = &read.maps[0];
			let change = curr_state.update(map, reads.len());
			let cstate = curr_state.0.as_ref().unwrap();
			match change {
				StateChange::NewBlock((x,y)) => {
					println!("Ending block ({}:{}-{} len = {}, {} maps)", hdr.tid2name(cstate.tid as usize), x, y, y - x + 1, reads.len());
					reads.clear();
					state_hash.clear();
				},
				StateChange::NewContig((tid, x, y)) => {
					println!("Ending contig with block ({}:{}-{} len = {}, {} maps)", hdr.tid2name(tid as usize), x, y, y - x + 1, reads.len());
					reads.clear();
					state_hash.clear();
				},
				StateChange::Init => {
					println!("Initiating new ")
				}
				_ => (),
			}	
			let id = brec.qname();
			let insert = if let Some(state) = state_hash.get(id) {
				match state {
					ReadState::Duplicate => {
						fs_stats.add_read_level_count(FSReadLevelType::Duplicate, brec.l_qseq() as usize);
						false
					},
					ReadState::Present(x) => {
						if map.is_last() {
							let rpair = reads[*x].as_mut().expect("Missing pair for read");
							let (filter, rflag, trimmed) = rpair.check_pair(&mut read, &bs_cfg.conf_hash);
							if trimmed > 0 {
								fs_stats.add_base_level_count(FSBaseLevelType::Overlapping, trimmed as usize);
							}
							if filter {
								fs_stats.add_read_level_count(rflag, brec.l_qseq() as usize);
								fs_stats.add_read_level_count(rflag, rpair.seq_qual.len());
								reads[*x] = None;
								false
							} else {
								// Get rid of reads that we have trimmed to zero length 
								if rpair.maps[0].rlen() == 0 { 
									fs_stats.add_read_level_count(rflag, rpair.seq_qual.len());
									reads[*x] = None 
								}
								if read.maps[0].rlen() == 0 {
									fs_stats.add_read_level_count(rflag, brec.l_qseq() as usize);
									false
								} else { true }
							}
						} else { true }
					}
				}
			} else {
				// Check if duplicate of already stored read
				if !keep_duplicates && read.check_dup(&reads[cstate.idx..]) {
					state_hash.insert(id.to_owned(), ReadState::Duplicate);
					false
				} else if map.is_last() {
					// println!("Inserting entry for {} at index {}", id, reads.len());
					state_hash.insert(id.to_owned(), ReadState::Present(reads.len()));
					true
				} else { true }
			};
			if insert { 
				fs_stats.add_read_level_count(FSReadLevelType::Passed, brec.l_qseq() as usize);
				reads.push(Some(read)) 
			};
		} else { fs_stats.add_read_level_count(read_flag, brec.l_qseq() as usize); }
	}
	for (flag, ct) in fs_stats.base_level().iter() { let _ = stat_tx.send(StatJob::AddFSBaseLevelCounts(*flag, *ct)); }
	for (flag, ct) in fs_stats.read_level().iter() { let _ = stat_tx.send(StatJob::AddFSReadLevelCounts(*flag, *ct)); }
	Ok(())
}

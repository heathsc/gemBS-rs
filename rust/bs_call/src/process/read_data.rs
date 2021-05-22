use std::{io, cmp, thread};
use std::sync::{Arc, mpsc};
use std::collections::HashMap;

use crate::htslib::*;
use crate::config::{BsCallConfig, BsCallFiles};
use super::records::{ReadEnd, Map};
use super::pileup;
use crate::stats::{StatJob, FSReadLevelType, FSType};

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
			} else if start_x > state.end_x + 2 { // Want at least 2 bases free between blocks to avoid issues with call context in VCF output
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

fn send_pileup_job(reads: Vec<Option<ReadEnd>>, cname: &str, x: u32, y: u32, tid: u32, pileup_tx: &mpsc::SyncSender<Option<pileup::PileupRegion>>) -> io::Result<()> {
	let preg = pileup::PileupRegion::new(cname, x as usize, y as usize, tid as usize, reads);
	match pileup_tx.send(Some(preg)) { 
		Err(e) => {
			warn!("Error trying to send new region to pileup thread");
			Err(hts_err(format!("Error sending region to pileup thread: {}", e)))
		},
		Ok(_) => Ok(()),
	} 	
}

fn 	count_passed_reads(reads: &[Option<ReadEnd>], fs_stats: &mut FSType) {
	for rd in reads.iter() {
		if let Some(read) = rd {
			if read.is_primary() { fs_stats.add_read_level_count(FSReadLevelType::Passed, read.seq_qual.len()); }
		}
	}
}

pub fn read_data(bs_cfg: Arc<BsCallConfig>, stat_tx: mpsc::Sender<StatJob>, mut bs_files: BsCallFiles) -> io::Result<()> {
	
	let mut sam_input = bs_files.sam_input.take().unwrap();
	let mut fs_stats = FSType::new();
	let (pileup_tx, pileup_rx) = mpsc::sync_channel(32);
	let cfg = Arc::clone(&bs_cfg);
	let st_tx = mpsc::Sender::clone(&stat_tx);
	let pileup_handle = thread::spawn(move || { pileup::make_pileup(Arc::clone(&bs_cfg), pileup_rx, bs_files, st_tx) });
	let hdr = &mut sam_input.hdr;

	let keep_duplicates = cfg.conf_hash.get_bool("keep_duplicates");
	let mut brec = BamRec::new().unwrap();
	let mut reads: Vec<Option<ReadEnd>> = Vec::new();
	let mut state_hash: HashMap<String, ReadState> = HashMap::new();
	let mut curr_state = State(None);
	loop {
		match sam_input.inner.get_next(&mut brec) {
			SamReadResult::Ok => (),
			SamReadResult::EOF => {
				if let Some(cstate) = curr_state.0.as_ref() {
					let cname = hdr.tid2name(cstate.tid as usize);
					let (x, y) = (cstate.start_x, cstate.end_x);
					trace!("Last block ({}:{}-{} len = {}, {} maps)", cname, x, y, y - x + 1, reads.len());
					count_passed_reads(&reads, &mut fs_stats);
					send_pileup_job(reads, cname, x, y, cstate.tid, &pileup_tx)?;
				}
				break;
			},
			_ => panic!("Error reading record"),
		}
		let (read_end, read_flag) = ReadEnd::from_bam_rec(&cfg.conf_hash, hdr, &brec);
		if let Some(mut read) = read_end {
			let map = &read.maps[0];
			let change = curr_state.update(map, reads.len());
			let cstate = curr_state.0.as_ref().unwrap();
			match change {
				StateChange::NewBlock((x,y)) => {
					let cname = hdr.tid2name(cstate.tid as usize);
					trace!("Ending block ({}:{}-{} len = {}, {} maps)", cname, x, y, y - x + 1, reads.len());
					count_passed_reads(&reads, &mut fs_stats);
					send_pileup_job(reads, cname, x, y, cstate.tid, &pileup_tx)?;
					reads = Vec::new();
					state_hash.clear();
				},
				StateChange::NewContig((tid, x, y)) => {
					let cname = hdr.tid2name(tid as usize);
					trace!("Ending contig with block ({}:{}-{} len = {}, {} maps)", cname, x, y, y - x + 1, reads.len());
					count_passed_reads(&reads, &mut fs_stats);
					send_pileup_job(reads, cname, x, y, tid, &pileup_tx)?;
					reads = Vec::new();
					state_hash.clear();
				},
				StateChange::Init => {
					debug!("Initiating run")
				}
				_ => (),
			}	
			let id = brec.qname();
			let insert = if let Some(state) = state_hash.get(id) {
				match state {
					ReadState::Duplicate => {
						if read.is_primary() { fs_stats.add_read_level_count(FSReadLevelType::Duplicate, brec.l_qseq() as usize); }
						false
					},
					ReadState::Present(x) => {
						if map.is_last() {
							let rpair = match reads[*x].as_mut() {
                                Some(rp) => rp,
                                None => panic!("Read {} appears multiple times\n", id),  
                            };
							let (filter, rflag) = rpair.check_pair(&mut read, &cfg.conf_hash);
							if filter {
								fs_stats.add_read_level_count(rflag, brec.l_qseq() as usize);
								fs_stats.add_read_level_count(rflag, rpair.seq_qual.len());
								reads[*x] = None;
								false
							} else {
								// Get rid of reads that we have trimmed to zero length 
								if rpair.maps[0].rlen() == 0 { 
									if rpair.is_primary() {
										fs_stats.add_read_level_count(FSReadLevelType::ZeroUnclipped, rpair.seq_qual.len());
									}
									reads[*x] = None 
								}
								if read.maps[0].rlen() == 0 {
									if read.is_primary() {
										fs_stats.add_read_level_count(FSReadLevelType::ZeroUnclipped, brec.l_qseq() as usize);
									}
									false
								} else { true }
							}
						} else { true }
					}
				}
			} else {
				// Check if duplicate of already stored read
				if !keep_duplicates && read.check_dup(&reads[cstate.idx..]) {
					if read.is_primary() { fs_stats.add_read_level_count(FSReadLevelType::Duplicate, brec.l_qseq() as usize); }
					state_hash.insert(id.to_owned(), ReadState::Duplicate);
					false
				} else if map.is_last() {
					// println!("Inserting entry for {} at index {}", id, reads.len());
					state_hash.insert(id.to_owned(), ReadState::Present(reads.len()));
					true
				} else { true }
			};
			if insert { reads.push(Some(read)) };
		} else { // Only collect stats on primary reads unless they are flagged for being secondary or supplementary
			if match read_flag {
				FSReadLevelType::SupplementaryAlignment | FSReadLevelType::SecondaryAlignment => true,
				_ => brec.flag() & (BAM_FSECONDARY | BAM_FSUPPLEMENTARY) == 0,
			} {	fs_stats.add_read_level_count(read_flag, brec.l_qseq() as usize) } 
		}
	}
	if pileup_tx.send(None).is_err() { warn!("Error trying to send QUIT signal to pileup thread") }
	else {
		for (flag, ct) in fs_stats.read_level().iter() { let _ = stat_tx.send(StatJob::AddFSReadLevelCounts(*flag, *ct)); }
		if pileup_handle.join().is_err() { warn!("Error waiting for pileup thread to finish") }
	}
	Ok(())
}

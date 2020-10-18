use crate::htslib::BSStrand;

// Set bit 2 for CA, CC or CT, and set bit 3 for AG, GG and TG
const RTAB: [u8; 64] = [
		0, 0, 0, 0, 0, 0, 0, 0,   // NX
		0, 0, 0, 8, 0, 0, 0, 0,   // AX
		0, 4, 4, 0, 4, 0, 0, 0,   // CX
		0, 0, 0, 8, 0, 0, 0, 0,   // GX
		0, 0, 0, 8, 0, 0, 0, 0,   // TX
		0, 0, 0, 0, 0, 0, 0, 0,   // NX
		0, 0, 0, 0, 0, 0, 0, 0,   // NX
		0, 0, 0, 0, 0, 0, 0, 0,   // NX
];

// Collect counts of conversion, mutation and non-conversion events in non-CpG contexts.  For each position in the read, we look
// at C and G positions not in CpG context (according to the reference).  We count C->C, C->T, G->G and G->A events separately on the different strands.
// We collect 4 counts per read position:
//    a = C->C on the G2A strand or G->G on the C2T strand (or either from a non-converted library)
//    b = C->T on the G2A strand or G->A on the C2T strand (or either from a non-converted library)
//    c = C->C on the C2T strand or G->G on the G2A strand
//    d = C->T on the C2T strand or G->A on the G2A strand
//
// We can use  b / (a + b) as an estimate of the sequencing error rate + mutation rate
// Assuming that non-CpG sites are truly non-methylated, then d / (c + d) is an estimate of conversion + sequencing error + mutation.
// so by combing both we can get an estimate of conversion per location on the read.
//
// We use a FSM to avoid branching.
// state has the previous and current reference bases as a 6 bit number, the high 3 bits code the previous base and the low 3 the current base
// 0 = N, 1 = A, 2 = C, 3 = G, 4 = T
// rtab[state] is either 4, 8 or 0.  If 4 then we have a C not followed by a G or an N, and if 8 we have a G not preceded by a C or an N
// The read vector has the combined quality and base information as a uint8_t.  The top 6 bits code the quality and the low 2 bits code the base.
// 0 = A, 1 = C, 2 = G, 3 = T.  N's have the quality set to 0 (and the base information is irrelevant)
// btab[] is a lookup table indexed on strand (as bits 8 and 9) combined with the read info above.  The encode value (xx) is a 4 bit number
// where the low 2 bits code for the count (0 = a, 1 = b, 2 = c, 3 = d), bit 2 corresponds to a potential valid count for a C/T base,
// while bit 3 corresponds to a potential valid count for a G/A base.  rtab[state] & btab[strand + read] will have either bit 2 or bit 3 set
// if we have a valid count.

pub struct MethProfile {
	profile: Vec<[usize; 4]>,
	table: [u8; 768],
}

impl MethProfile {
	pub fn new(min_qual: usize) -> Self {
		assert!(min_qual < 64);
		let mut table = [0; 768];
		for q in min_qual..64 {
			let x = q << 2;
			// Non converted
			table[x] = 11; 
			table[x + 1] = 6; 
			table[x + 2] = 10; 
			table[x + 3] = 7;
			// C2T
			table[x + 256] = 11;
			table[x + 257] = 4;
			table[x + 258] = 10;
			table[x + 259] = 5;
			// G2A
			table[x + 512] = 9;
			table[x + 513] = 6;
			table[x + 514] = 8;
			table[x + 515] = 7;	
		}
		let profile = Vec::new();
		Self{ table, profile }
	}

	pub fn add_profile(&mut self, ref_seq: &[u8], mut opos: isize, mut state: u8, sq: &[u8], rev: bool, bs: BSStrand) {
		assert!(!sq.is_empty());
		let (d, max_pos) = if rev { (-1, (opos as usize)) } else { (1, (opos as usize) + sq.len() - 1) };
		if max_pos >= self.profile.len() { self.profile.resize(max_pos + 1, [0, 0, 0, 0]) }
		let mtab =  match bs {
			BSStrand::StrandC2T => &self.table[256..512],
			BSStrand::StrandG2A => &self.table[512..],
			_ => &self.table[0..256],
		};
		let mut ref_iter = ref_seq.iter();
		state = ((state << 3) | ref_iter.next().unwrap_or(&0)) & 63;
		let mut mask = RTAB[state as usize];
		for x in sq.iter().map(|c| mtab[*c as usize]) {
			let mask1 = (x & mask) >> 1;
			state = ((state << 3) | ref_iter.next().unwrap_or(&0)) & 63;
			mask = RTAB[state as usize];
			self.profile[opos as usize][(x & 3) as usize] += ((((x & mask) | mask1) >> 2) & 1) as usize;
			opos += d;
		}
	}

	pub fn take_profile(self) -> Vec<[usize; 4]> {
		let MethProfile{profile, ..} = self;
		profile
	}
}


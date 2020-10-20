use std::cmp;
use crate::process::GT_HET;

use libc::c_double;

#[link(name = "m")]
extern "C" {
	fn lgamma(x: c_double) -> c_double;
}

const LFACT_STORE_SIZE: usize = 256;

pub struct FisherTest {
	lfact_store: [f64; LFACT_STORE_SIZE],
}

impl FisherTest {
	pub fn new() -> Self {
		let mut lfact_store = [0.0; LFACT_STORE_SIZE];
		for i in 2..LFACT_STORE_SIZE {
			lfact_store[i] = lfact_store[i - 1] + (i as f64).ln();
		} 
		Self{lfact_store}
	}
	
	pub fn calc_fs_stat(&self, mx: usize, cts: &[u32], ln_10: f64) -> f64 {
		if GT_HET[mx] {
			let get_cts = |x: &[usize], y: &[usize]| [x.iter().fold(0, |s, x| s + cts[*x]), y.iter().fold(0, |s, x| s + cts[*x]),
				x.iter().fold(0, |s, x| s + cts[*x + 8]), y.iter().fold(0, |s, x| s + cts[*x + 8])];
			
			let ftab: [u32; 4] = match mx {
				1 => get_cts(&[0, 4], &[1, 5, 7]), // AC
				2 => get_cts(&[0], &[2, 6]), // AG
				3 => get_cts(&[0, 4], &[3, 7]), // AT
				5 => get_cts(&[1, 5, 7], &[2, 4, 6]), // CG
				6 => get_cts(&[1, 5], &[3]), // CT
				8 => get_cts(&[2,4,6], &[3, 7]), // GT
				_ => panic!("Unexpected genotype"),
			};
			let z = self.fisher(&ftab);
			(if z < 1.0e-20 { 1.0e-20 } else { z }).ln() / ln_10
		} else { 0.0 } 
	}	
	
	pub fn lfact(&self, x: usize) -> f64 { if x < LFACT_STORE_SIZE { self.lfact_store[x] } else { unsafe { lgamma((x + 1) as f64) }}}
	
	pub fn fisher(&self, ftab: &[u32; 4]) -> f64 {
		let row = [(ftab[0] + ftab[1]) as f64, (ftab[2] + ftab[3]) as f64];
		let col = [(ftab[0] + ftab[2]) as f64, (ftab[1] + ftab[3]) as f64];
		let n = row[0] + row[1];
		let mut c: Vec<usize> = ftab.iter().map(|x| *x as usize).collect();
		if n < 1.0 { 1.0 }
		else {
			let delta = (ftab[0] as f64) - row[0] * col[0] / n;
			let konst = self.lfact(c[0] + c[2]) + self.lfact(c[1] + c[3]) + self.lfact(c[0] + c[1]) + self.lfact(c[2] + c[3]) - self.lfact(c[0] + c[1] + c[2] + c[3]);
			let mut like = (konst - self.lfact(c[0]) - self.lfact(c[1]) - self.lfact(c[2]) - self.lfact(c[3])).exp();
			let mut prob = like;
			if delta > 0.0 {
				// Decrease counter diagonal elements until zero (this will increase delta)
				let min = cmp::min(c[1], c[2]);
				for i in 0..min { 
					like *= (((c[1] - i) * (c[2] - i)) as f64) / (((c[0] + i + 1) * (c[3] + i + 1)) as f64);
					prob += like;
				}
				let min = cmp::min(c[0], c[3]);
				// Calculate amount required to increase delta by decreasing leading diagonal elements
				let adjust = (2.0 * delta).ceil() as usize;
				if adjust <= min {
					c[0] -= adjust;
					c[3] -= adjust;
					c[1] += adjust;
					c[2] += adjust;
					like = (konst - self.lfact(c[0]) - self.lfact(c[1]) - self.lfact(c[2]) - self.lfact(c[3])).exp();
					prob += like;
					for i in 0..min-adjust {
						like *= (((c[0] - i) * (c[3] - i)) as f64) / (((c[1] + i + 1) * (c[2] + i + 1)) as f64);
						prob += like;					
					}
				}
			} else {
				// Decrease leading diagonal elements until zero (this will increase delta)
				let min = cmp::min(c[0], c[3]);
				for i in 0..min { 
					like *= (((c[0] - i) * (c[3] - i)) as f64) / (((c[1] + i + 1) * (c[2] + i + 1)) as f64);
					prob += like;
				}
				let min = cmp::min(c[1], c[2]);
				let adjust = cmp::max((-2.0 * delta).ceil() as usize, 1);
				if adjust <= min {
					c[0] += adjust;
					c[3] += adjust;
					c[1] -= adjust;
					c[2] -= adjust;
					like = (konst - self.lfact(c[0]) - self.lfact(c[1]) - self.lfact(c[2]) - self.lfact(c[3])).exp();
					prob += like;
					for i in 0..min-adjust {
						like *= (((c[1] - i) * (c[2] - i)) as f64) / (((c[0] + i + 1) * (c[3] + i + 1)) as f64);
						prob += like;					
					}
				}
			}
			prob
		}
	}
}





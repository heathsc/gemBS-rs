pub struct QualProb {
	k: f64,
	ln_k: f64,
	ln_k_half: f64,
	ln_k_one: f64,
}

pub struct Model {
	qtab: Vec<QualProb>,
	ln_ref_bias: f64,
	ln_ref_bias_1: f64,
	lambda: f64, // 1 - under_conversion rate
	theta: f64, // over_conversion rate
	ln_10: f64,
}

impl Model {
	pub fn new(max_qual: usize, conv: (f64, f64), ref_bias: f64) -> Self {
		assert!(conv.0 > 0.0 && conv.0 < 1.0 && conv.1 > 0.0 && conv.1 < 1.0 && ref_bias > 0.0);
		let mut v = Vec::with_capacity(max_qual + 1);
		let ln_10 = 10.0_f64.ln();
		for q in 0..=max_qual {
			let e = {
				let t = ((q as f64) * ln_10 * -0.1).exp();
				if t > 0.5 { 0.5 } else { t }
			};
			let k = e / (3.0 - 4.0 * e);
			v.push(QualProb{
				k, 
				ln_k: k.ln(),
				ln_k_half: (k + 0.5).ln(),
				ln_k_one: k.ln_1p(),
			})
		}
		Self{qtab: v, lambda: 1.0 - conv.0, theta: conv.1, ln_ref_bias: ref_bias.ln(), ln_ref_bias_1:(0.5 * (1.0 + ref_bias)).ln(), ln_10 }
	}
	pub fn ln_10(&self) -> f64 { self.ln_10 }

  /*********************************************************************************************
   * Base and methylation frequencies are described by 5 parameters: w, p, q, mc, mg
   * 
   * Let n(X) be the count for base X, and N the total number of bases seen
   * w = (n(C) + n(T)) / N
   * p = n(C) / (n(C) + n(T))
   * q = n(G) / (n(A) + n(G))
   * mc is the proportion of methylated Cs on the top strand
   * mg is the proportion of methylated Cs on the bottom strand
   *
   * Base frequencies are therefore:
   *  f(A) = (1 - w) * (1 - q)
   *  f(C) = w * p
   *  f(G) = (1 - w) * q
   *  f(T) = w * (1 - p)
   *
   * All 5 parameters are ratios are are therefore independently constrained 
   * to be between 0 and 1.
   * 
   * We first maximize the full model, allowing w, p, q, mc and mg to take 
   * any legal value.  The log likelihood of this model is l_full.
   *
   * We then calculate the marginal likelihood for the 10 possible genetic models compatible
   * with a diploid state (thereby fixing w, p, q) and maximizing the likelihood over (mc, mg).
   *
   * The called genotype is that with the highest likelihood 
   * The phred score is calculated as the phred scaled posterior genotype probability (considering
   * only the 10 possible diploid genotypes)
   *
   **********************************************************************************************/	
	pub fn calc_gt_prob(&self, counts: &[usize; 8], qual: &[usize; 8], ref_base: u8) -> (usize, [f64; 10]) {
		let qp: Vec<_> = qual.iter().map(|x| &self.qtab[*x]).collect();
		let n: Vec<_> = counts.iter().map(|x| *x as f64).collect();
		let mut ll = self.add_ref_prior(ref_base);
		let get_par = |i: usize| (n[i] * qp[i].ln_k_one, n[i] * qp[i].ln_k_half, n[i] * qp[i].ln_k);		
		let mut add_contrib = |v: &[f64]| ll.iter_mut().zip(v.iter()).for_each(|(l, x)| *l += *x);
		if counts[0] != 0 {
			let (x, tz, tz1) = get_par(0);
			add_contrib(&[x, tz, tz, tz, tz1, tz1, tz1, tz1, tz1, tz1]);
		}
		if counts[1] != 0 {
			let (x, tz, tz1) = get_par(1);
			add_contrib(&[tz1, tz, tz1, tz1, x, tz, tz, tz1, tz1, tz1]);
		}
		if counts[2] != 0 {
			let (x, tz, tz1) = get_par(2);
			add_contrib(&[tz1, tz1, tz, tz1, tz1, tz, tz1, x, tz, tz1]);
		}
		if counts[3] != 0 {
			let (x, tz, tz1) = get_par(3);
			add_contrib(&[tz1, tz1, tz1, tz, tz1, tz1, tz, tz1, tz, x]);
		}
		let z0 = if counts[0] + counts[1] != 0 { self.get_z( n[5], n[7], qp[5].k, qp[7].k) } else { (-1.0, -1.0, -1.0) };
		let z1 = if counts[4] + counts[6] != 0 { self.get_z( n[6], n[4], qp[6].k, qp[4].k) } else { (-1.0, -1.0, -1.0) };
		if counts[4] != 0 {
			let (x, tz, tz1) = get_par(4);
			let tz2 = n[4] * (0.5 * (1.0 - z1.2) + qp[4].k).ln();
			add_contrib(&[x, tz, n[4] * (1.0 - 0.5 * z1.1 + qp[4].k).ln(), tz, tz1, tz2, tz1, n[4] * (1.0 - z1.0 + qp[4].k).ln(), tz2, tz1]);
		}
		if counts[5] != 0 {
			let (tz, tz1) = (n[5] * (0.5 * z0.2 + qp[5].k).ln(), n[5] * qp[5].ln_k);
			add_contrib(&[tz1, tz, tz1, tz1, n[5] * (z0.0 + qp[5].k).ln(), tz, n[5] * (0.5 * z0.1 + qp[5].k).ln(), tz1, tz1, tz1]);
		}
		if counts[6] != 0 {
			let (tz, tz1) = (n[6] * (0.5 * z1.2 + qp[6].k).ln(), n[6] * qp[6].ln_k);
			add_contrib(&[tz1, tz1, n[6] * (0.5 * z1.1 + qp[6].k).ln(), tz1, tz1, tz, tz1, n[6] * (z1.0 + qp[6].k).ln(), tz, tz1]);
		}
		if counts[7] != 0 {
			let (x, tz, tz1) = get_par(7);
			let tz2 = n[7] * (0.5 * (1.0 - z0.2) + qp[7].k).ln();
			add_contrib(&[tz1, tz2, tz1, tz, n[7] * (1.0 - z1.0 + qp[7].k).ln(), tz2, n[7] * (1.0 - 0.5 * z1.1 + qp[7].k).ln(), tz1, tz, x]);
		}
		let (mx, max) = ll[1..].iter().cloned().enumerate().fold((0, ll[0]), |(i, m), (j, l)| if l > m { (j, l) } else { (i, m) });
		let sum = (ll.iter().cloned().fold(0.0, |s, x| s + (x - max).exp())).ln();
		ll.iter_mut().for_each(|x| *x = (*x - max - sum) / self.ln_10);
		(mx, ll)
	}

	fn get_z(&self, x1: f64, x2: f64, k1: f64, k2: f64) -> (f64, f64, f64) {
		let lpt = self.lambda + self.theta;
		let lmt = self.lambda - self.theta;
		let d = (x1 + x2) * lmt;
		let f = |x| if x < 1.0 { 1.0 - self.lambda } else if x > 1.0 { 1.0 - self.theta } else { 0.5 * (lmt * x + 2.0 - lpt) };
		(
			f((x1 * (lpt + 2.0 * k2) - x2 * (2.0 - lpt + 2.0 * k1)) / d), // w = 1, p = 1
			f((x1 * (2.0 + lpt + 4.0 * k2) - x2 * (2.0 - lpt + 4.0 * k1)) / d), // w = 1, p = 1/2
			f((x1 * (lpt + 4.0 * k2) - x2 * (2.0 - lpt + 4.0 * k1)) / d) // w = 1.2, p = 1
		)
	}

	fn add_ref_prior(&self, ref_base: u8) -> [f64; 10] {
		let mut ll = [0.0; 10];
		let (lrb, lrb1) = (self.ln_ref_bias, self.ln_ref_bias_1);
		match ref_base {
			1 => { // A
				ll[0] = lrb;
				ll[1] = lrb1;
				ll[2] = lrb1;
				ll[3] = lrb;
			},
		    2 => { // C
				ll[1] = lrb1;
				ll[4] = lrb;
				ll[5] = lrb1;
				ll[6] = lrb1;
			},
			3 => { // G
				ll[2] = lrb1;
				ll[5] = lrb1;
				ll[7] = lrb;
				ll[8] = lrb1;
			},
			4 => { // T
				ll[3] = lrb1;
				ll[6] = lrb1;
				ll[8] = lrb1;
				ll[9] = lrb;
			},
			_ => (), // N
		}
		ll
	}
}

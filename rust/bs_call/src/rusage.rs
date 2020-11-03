use std::{mem, fmt};

pub struct Rusage {
	inner: libc::rusage,
}

pub enum RusageWho { RusageSelf, RusageChildren, RusageThread }

impl RusageWho {
	pub fn get_c_val(&self) -> libc::c_int {
		match self {
			RusageWho::RusageSelf => libc::RUSAGE_SELF,
			RusageWho::RusageChildren => libc::RUSAGE_CHILDREN,
			RusageWho::RusageThread => libc::RUSAGE_THREAD,
		}
	}
}

pub struct Timeval {
	tv: libc::timeval,
}

impl fmt::Display for Timeval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut tm = self.tv.tv_sec as f64 + (self.tv.tv_usec as f64) / 1000000.0;
		let sign = if tm < 0.0 { "-" } else { "" };
		tm = tm.abs();
		let hours = (tm / 3600.0).round();
		let min = ((tm - hours * 3600.0) / 60.0).round();
		let sec = tm - hours * 3600.0 - min * 60.0;
		if hours > 0.0 {
        	write!(f, "{}{}h{}m{:.3}s", sign, hours, min, sec)
		} else {
			write!(f, "{}{}m{:.3}s", sign, min, sec)
		}
    }
}

impl Rusage {
	pub fn get(who: RusageWho) -> Result<Self, &'static str> {
		let who = who.get_c_val();
		unsafe {
			let mut r: libc::rusage = mem::MaybeUninit::zeroed().assume_init();
			if  libc::getrusage(who, &mut r) == 0 {
				Ok(Self{inner: r})
			} else { Err("getrusage() failed - not implemented")}
		}
	}
	pub fn update(&mut self, who: RusageWho) {
		let who = who.get_c_val();
		unsafe { assert!(libc::getrusage(who, &mut self.inner) == 0); }		
	}
	pub fn utime(&self) -> Timeval { Timeval{tv: self.inner.ru_utime} }
	pub fn stime(&self) -> Timeval { Timeval{tv: self.inner.ru_stime} }
}


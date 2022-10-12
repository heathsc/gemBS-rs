use crate::config::*;
use std::io;

use clap::ArgMatches;

pub fn get_option(m: &ArgMatches, opt: &str, default: ConfVar) -> io::Result<ConfVar> {
    match default {
        ConfVar::Bool(_) => {
            if m.contains_id(opt) {
                Ok(ConfVar::Bool(true))
            } else {
                Ok(default)
            }
        }
        ConfVar::String(_) => m
            .get_one::<String>(opt)
            .map(|x| Ok(ConfVar::String(Some(x.to_owned()))))
            .unwrap_or(Ok(default)),
        ConfVar::Float(_) => m
            .get_one::<f64>(opt)
            .map_or_else(|| Ok(default), |x| Ok(ConfVar::Float(*x))),
        ConfVar::Int(_) => m
            .get_one::<usize>(opt)
            .map_or_else(|| Ok(default), |x| Ok(ConfVar::Int(*x))),
        ConfVar::Mode(_) => {
            let s = m.get_one::<String>(opt).map(|x| x.to_ascii_lowercase());
            match s.as_deref() {
                Some("combined") => Ok(ConfVar::Mode(Mode::Combined)),
                Some("strand-specific") => Ok(ConfVar::Mode(Mode::StrandSpecific)),
                Some(s) => Err(new_err(format!(
                    "Couldn't parse argument '{}' for option {}",
                    s, opt
                ))),
                None => Ok(default),
            }
        }
        ConfVar::Select(_) => {
            let s = m.get_one::<String>(opt).map(|x| x.to_ascii_lowercase());
            match s.as_deref() {
                Some("hom") => Ok(ConfVar::Select(Select::Hom)),
                Some("het") => Ok(ConfVar::Select(Select::Het)),
                Some(s) => Err(new_err(format!(
                    "Couldn't parse argument '{}' for option {}",
                    s, opt
                ))),
                None => Ok(default),
            }
        }
    }
}

pub fn get_fvec(m: &ArgMatches, opt: &str, low: f64, high: f64) -> io::Result<Option<Vec<f64>>> {
    if let Some(v) = m.get_many::<f64>(opt) {
        let mut vec = Vec::new();
        for z in v.copied() {
            if z >= low && z <= high {
                vec.push(z)
            } else {
                return Err(new_err(format!(
                    "Float argument '{}' for option {} not between {} and {}",
                    z, opt, low, high
                )));
            }
        }
        Ok(Some(vec))
    } else {
        Ok(None)
    }
}

pub fn get_f64(m: &ArgMatches, opt: &str, low: f64, high: f64) -> io::Result<Option<f64>> {
    if let Some(z) = m.get_one::<f64>(opt).copied() {
        if z >= low && z <= high {
            Ok(Some(z))
        } else {
            return Err(new_err(format!(
                "Float argument {} for option {} not between {} and {}",
                z, opt, low, high
            )));
        }
    } else {
        Ok(None)
    }
}

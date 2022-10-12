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
        ConfVar::OType(_) => m
            .get_one::<OType>(opt)
            .map_or_else(|| Ok(default), |x| Ok(ConfVar::OType(*x))),
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

pub fn get_ivec(
    m: &ArgMatches,
    opt: &str,
    low: usize,
    high: usize,
) -> io::Result<Option<Vec<usize>>> {
    if let Some(v) = m.get_many::<usize>(opt) {
        let mut vec = Vec::new();
        for z in v.copied() {
            if z >= low && z <= high {
                vec.push(z)
            } else {
                return Err(new_err(format!(
                    "Integer argument '{}' for option {} not between {} and {}",
                    z, opt, low, high
                )));
            }
        }
        Ok(Some(vec))
    } else {
        Ok(None)
    }
}

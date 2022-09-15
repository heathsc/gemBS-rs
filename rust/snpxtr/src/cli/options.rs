use clap::{ArgMatches, ErrorKind};
use std::collections::HashSet;
use std::io;

use r_htslib::*;
use utils::compress;

use crate::config::{new_err, Config, OutputOpt};
use crate::dbsnp;

fn read_select_file(s: &str) -> io::Result<HashSet<String>> {
    let mut sel_set = HashSet::new();
    let mut rdr = compress::open_bufreader(s)?;
    info!("Reading selected SNP list from {}", s);
    let mut buf = String::with_capacity(256);
    loop {
        buf.clear();
        let l = rdr.read_line(&mut buf)?;
        if l == 0 {
            break;
        }
        if let Some(sname) = buf.split_ascii_whitespace().next() {
            if let Some(name) = sname.strip_prefix("rs") {
                sel_set.insert(name.to_owned());
            } else {
                sel_set.insert(sname.to_owned());
            }
        }
    }
    info!("Read in {} unique SNP IDs", sel_set.len());
    Ok(sel_set)
}

pub fn handle_options(m: &ArgMatches) -> io::Result<Config> {
    let mut output_opt = OutputOpt::new();
    match m.value_of("output") {
        Some(s) => output_opt.set_filename(s),
        None => &mut output_opt,
    }
    .set_compress(m.is_present("compress"))
    .set_compute_md5(m.is_present("md5"))
    .set_compute_tbx(m.is_present("tabix"))
    .fix_opts();
    let mut sr = BcfSrs::new()?;
    let infile = m.value_of("input").expect("No input filename"); // This should not be allowed by Clap
    let regions = {
        if let Some(mut v) = m
            .values_of("regions")
            .or_else(|| m.values_of("region_list"))
        {
            let s = v.next().unwrap().to_owned();
            Some((
                v.fold(s, |mut st, x| {
                    st.push(',');
                    st.push_str(x);
                    st
                }),
                false,
            ))
        } else if let Some(s) = m.value_of("regions_file") {
            Some((s.to_owned(), true))
        } else {
            None
        }
    };
    if let Some((reg, flag)) = regions {
        sr.set_regions(&reg, flag)?
    }
    let nt = match value_t!(m, "threads", usize) {
        Ok(x) => {
            if x > 0 {
                sr.set_threads(x)?
            }
            Some(x)
        }
        Err(e) if e.kind == ErrorKind::ArgumentNotFound => None,
        Err(e) => return Err(new_err(format!("Error parsing option: {}", e))),
    };
    sr.add_reader(infile)?;
    let ns = sr.get_reader_hdr(0)?.nsamples();
    if ns == 0 {
        return Err(new_err(format!("No samples in input file {}", infile)));
    }

    let mut conf = Config::new(output_opt, sr);
    if let Some(n) = nt {
        conf.set_threads(n);
    }
    if let Some(s) = m.value_of("dbsnp") {
        let dbsnp_index = dbsnp::DBSnpIndex::new(s)?;
        conf.set_dbsnp_file(dbsnp::DBSnpFile::open(dbsnp_index)?);
    }
    if let Some(s) = m.value_of("selected") {
        conf.set_selected_hash(read_select_file(s)?);
    }

    Ok(conf)
}

use clap::ArgMatches;
use crate::common::defs::{Section, VarType};

mod config;

pub fn prepare_command(m: &ArgMatches) -> Result<(), String> {
	
	// We can just unwrap here because we should only get here if the config option is present,
	// so if it is not present then there has been an internal error an we can panic...
	let config = m.value_of("config").unwrap();
	println!("config: {}", config);
	let mut prep_config = config::PrepConfig::new();
/*	let x = prep_config.check_vtype("bcf_dir", Section::Calling);
	let y = prep_config.check_vtype("bcf_dir", Section::Mapping);
	let z = prep_config.check_vtype("threads", Section::Report);
	println!("x: {:?}", x);
	println!("y: {:?}", y);
	println!("z: {:?}", z); */
	prep_config.start_parse(config)?;
	prep_config.parse()?;
	Ok(())
}

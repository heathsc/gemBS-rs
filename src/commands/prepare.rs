use clap::ArgMatches;

pub fn prepare_command(m: &ArgMatches) -> Result<(), &'static str> {
	// We can just unwrap here because we should only get here if the config option is present,
	// so if it is not present then there has been an internal error an we can panic...
	let config = m.value_of("config").unwrap();
	println!("config: {}", config);
	Ok(())
}

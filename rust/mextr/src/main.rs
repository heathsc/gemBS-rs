#[macro_use]
extern crate log;

pub mod bbi;
mod cli;
pub mod config;
pub mod output;
pub mod process;
pub mod read_vcf;

fn main() -> Result<(), String> {
    let (chash, sr) = cli::process_cli()
        .map_err(|e| format!("mextr_index initialization failed with error: {}", e))?;
    match process::process(chash, sr) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("mextr failed with error: {}", e);
            Err("Failed".to_string())
        }
    }
}

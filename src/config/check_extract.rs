// Check requirements and presence of source and derived files for extraction
// Make asset list for BCFs, BED, BigWig etc. associated with traction

use std::path::{Path, PathBuf};
use std::rc::Rc;
use crate::common::defs::{Section, DataValue, Command, ContigInfo};
use crate::common::assets::{AssetType, GetAsset};
use super::GemBS;

pub fn check_extract(gem_bs: &mut GemBS) -> Result<(), String> {
	
	Ok(())
}
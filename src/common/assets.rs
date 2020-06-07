use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, Copy)]
pub enum AssetType { Supplied, Derived, Temp, Log }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssetStatus { Present, Absent, Incomplete }

#[derive(Debug, Clone)]
pub struct Asset {
	id: Rc<String>,
	path: PathBuf,
	idx: usize,
	creator: Option<usize>,
	asset_type: AssetType,
	status: AssetStatus,
}

impl Asset {
	fn new(id_str: &str, path: &Path, idx: usize, asset_type: AssetType) -> Self {
		let status = if path.exists() { AssetStatus::Present } else { 
			if let AssetType::Supplied = asset_type { 
				warn!("Warning: datafile {} required for analysis is not present or not accessible", path.to_string_lossy());
			}
			AssetStatus::Absent 
		};
		let id = Rc::new(id_str.to_owned());		
		Asset{id, path: path.to_owned(), idx, creator: None, asset_type, status}
	}	
	pub fn path(&self) -> &Path { &self.path }
	pub fn status(&self) -> AssetStatus { self.status }
	pub fn idx(&self) -> usize { self.idx }
	pub fn creator(&self) -> Option<usize> { self.creator }
	pub fn set_creator(&mut self, idx: usize) { self.creator = Some(idx); }
}

pub struct AssetList {
	asset_hash: HashMap<Rc<String>, usize>, 
	assets: Vec<Asset>,
}

pub trait GetAsset<T> {
	fn get_asset(&self, idx: T) -> Option<&Asset>; 	
	fn get_asset_mut(&mut self, idx: T) -> Option<&mut Asset>; 	
}

impl GetAsset<usize> for AssetList {
	fn get_asset(&self, idx: usize) -> Option<&Asset> {
		if idx < self.assets.len() { Some(&self.assets[idx]) }
		else { None }
	}
	fn get_asset_mut(&mut self, idx: usize) -> Option<&mut Asset> {
		if idx < self.assets.len() { Some(&mut self.assets[idx]) }
		else { None }
	}
}


impl GetAsset<&str> for AssetList {
	fn get_asset(&self, idx: &str) -> Option<&Asset> {
		self.asset_hash.get(&idx.to_string()).map(|x| &self.assets[*x])
	}
	fn get_asset_mut(&mut self, idx: &str) -> Option<&mut Asset> {
		if let Some(x) = self.asset_hash.get(&idx.to_string()) {
			let ix = *x;
			Some(&mut self.assets[ix])		
		} else { None }
	}
}
	
impl AssetList {
	pub fn new() -> Self { AssetList{asset_hash: HashMap::new(), assets: Vec::new() }}

	pub fn insert(&mut self, id: &str, path: &Path, asset_type: AssetType) -> usize {
		let idx = self.assets.len();
		let asset = Asset::new(id, path, idx, asset_type);
		let asset_id = Rc::clone(&asset.id);
		self.assets.push(asset);
		self.asset_hash.insert(asset_id, idx);
		idx
	}

}
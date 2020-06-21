use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::SystemTime;

use super::utils::calc_digest;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssetType { Supplied, Derived, Temp, Log }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssetStatus { Present, Outdated, Absent }

#[derive(Debug, Clone)]
pub struct Asset {
	id: Rc<String>,
	path: PathBuf,
	idx: usize,
	creator: Option<usize>,
	parents: Vec<usize>,
	asset_type: AssetType,
	status: AssetStatus,
	mod_time: Option<SystemTime>,
	mod_time_ances: Option<SystemTime>,
}

fn get_status_time(path: &Path, asset_type: AssetType) -> (AssetStatus, Option<SystemTime>) {
	match path.metadata() {
		Ok(md) => {
			(AssetStatus::Present, md.modified().ok())
		},
		Err(e) => {
			if let AssetType::Supplied = asset_type {warn!("Warning: required datafile {} not accessible: {}", path.to_string_lossy(), e)}
			(AssetStatus::Absent, None)
		},
	}
}
impl Asset {
	fn new(id_str: &str, path: &Path, idx: usize, asset_type: AssetType) -> Self {
		let (status, mod_time) = get_status_time(path, asset_type);
		let id = Rc::new(id_str.to_owned());		
		Asset{id, path: path.to_owned(), idx, creator: None, parents: Vec::new(), asset_type, status, mod_time, mod_time_ances: mod_time}
	}
	pub fn recheck_status(&mut self) {
		let (status, mod_time) = get_status_time(&self.path, self.asset_type);
		self.status = status;
		self.mod_time = mod_time;
	}	
	pub fn path(&self) -> &Path { &self.path }
	pub fn status(&self) -> AssetStatus { self.status }
	pub fn idx(&self) -> usize { self.idx }
	pub fn id(&self) -> &str { &self.id }
	pub fn creator(&self) -> Option<usize> { self.creator }
	pub fn set_creator(&mut self, idx: usize, pvec: &[usize]) { 
		self.creator = Some(idx);
		pvec.iter().for_each(|x| self.parents.push(*x)); 
	}
	pub fn mod_time(&self) -> Option<SystemTime> { self.mod_time }
	pub fn mod_time_ances(&self) -> Option<SystemTime> { self.mod_time_ances }
	pub fn parents(&self) -> &[usize] { &self.parents }
	pub fn asset_type(&self) -> AssetType { self.asset_type }
}

pub fn make_log_asset(id: &str, par: &Path) -> (String, PathBuf) {
	let lname = format!("{}.log", id);
	let lpath: PathBuf = [par, Path::new(&lname)].iter().collect();
	(lname, lpath) 
}

pub fn derive_log_asset(id: &str, file: &Path) -> (String, PathBuf) {
	let mut v = Vec::new();
	if let Some(par) = file.parent() { v.push(par) }
	let lname = format!("{}.log", id);
	v.push(Path::new(lname.as_str()));
	let lpath: PathBuf = v.iter().collect();
	(lname, lpath) 
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
		if let Some(a) = self.get_asset(id) {
			warn!("Warning - Can not insert asset {}, path {} as asset already exists with path {}", id, path.to_string_lossy(), a.path().to_string_lossy());
			a.idx()
		} else {		
			let idx = self.assets.len();
			let asset = Asset::new(id, path, idx, asset_type);
			let asset_id = Rc::clone(&asset.id);
			self.assets.push(asset);
			self.asset_hash.insert(asset_id, idx);
			idx
		}
	}
		
	// Make a digest from all of the Asset names.  We sort them as the standard HashMap helpfully randomizes
	// the order.
	pub fn get_digest(&self) -> String {
		calc_digest(itertools::sorted(self.assets.iter().map(|x| x.id.as_bytes()).collect::<Vec<_>>()))
	}
	
	fn calc_mta(&self, idx: usize, visited: &mut Vec<bool>, mtime: &mut Vec<Option<SystemTime>>) {
		if !visited[idx] {
			let asset = &self.assets[idx];
			trace!("Getting mod_time_ances of {:?}", asset);
			if let AssetType::Supplied = asset.asset_type {	mtime[idx] = asset.mod_time; }
			else {
				let cmp_time = |x: Option<SystemTime>, y: Option<SystemTime>| match (x, y) {
					(None, None) => None,
					(Some(m), None) => Some(m),
					(None, Some(m)) => Some(m),
					(Some(m), Some(n)) => if n > m { Some(n) } else { Some(m) }									
				};
				let mut latest_time = None;
				for j in &asset.parents {
					self.calc_mta(*j, visited, mtime);
					latest_time = cmp_time(latest_time, mtime[*j]);
				}
				mtime[idx] = cmp_time(latest_time, asset.mod_time);
			}
			visited[idx] = true;
		} 
	}
	
	pub fn recheck_status(&mut self) {
		for asset in self.assets.iter_mut() { asset.recheck_status() }
	}
	
	pub fn calc_mod_time_ances(&mut self) {
		let len = self.assets.len();
		let mut visited = vec!(false; len);
		let mut mtime: Vec<Option<SystemTime>> = vec!(None; len);
		// recurse through tree, checking supplied assets before derived ones
		for ix in 0..len { self.calc_mta(ix, &mut visited, &mut mtime); }
		for (ix, asset) in self.assets.iter_mut().enumerate() { 
			asset.mod_time_ances = mtime[ix];
			if let AssetStatus::Present = asset.status {
				if let (Some(m), Some(n)) = (asset.mod_time, asset.mod_time_ances) {
					if n > m { asset.status = AssetStatus::Outdated; }
				}
			} 
		}
	}
}


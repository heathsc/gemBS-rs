#[macro_use]
extern crate lazy_static;

use std::env;
use std::ffi::{CString, OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub mod compress;
pub mod log_level;

fn access(p: &Path) -> Result<bool, String> {
    let cstr = CString::new(p.as_os_str().as_bytes())
        .map_err(|e| format!("access(): error converting {}: {}", p.display(), e))?;
    unsafe { Ok(libc::access(cstr.as_ptr(), libc::X_OK) == 0) }
}

pub fn find_exec_path<S: AsRef<OsStr>>(prog: S) -> Option<PathBuf> {
    let search_path =
        env::var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/usr/local/bin"));
    for path in env::split_paths(&search_path) {
        let candidate = path.join(prog.as_ref());
        if candidate.exists() {
            if let Ok(true) = access(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

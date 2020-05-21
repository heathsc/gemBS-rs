use std::ffi::CString;

pub fn get_inode(name: &str) -> Option<u64> {
	match CString::new(name) {
		Ok(cname) => unsafe {
			let mut buf = std::mem::MaybeUninit::<libc::stat>::uninit();
			if libc::stat(cname.as_ptr(), buf.as_mut_ptr()) == 0 {
				let buf = buf.assume_init();
				Some(buf.st_ino)
			} else {
				error!("Could not access file {}", name);
				None 
			}
		},
		Err(_) => {
			error!("get_inode() failed for {}", name);
			None
		},
	}	
}

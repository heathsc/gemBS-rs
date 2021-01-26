use std::ptr::NonNull;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

///
/// MallocDataBlock<T>
/// 
/// Intended for data that will be shared with a C API where allocation
/// and reallocation of a buffer is performed within C (using malloc/realloc)
/// via a C function such as:
///   len = c_func(void **mem, int *size, ...)
/// 
/// where mem is initialized to null and size to 0, and on return
/// mem and size will be set and len will give the number of valid values
/// Subsequent calls to c_func may call realloc on mem (so it might change).
/// 
/// # Examples
/// 
/// ...
/// let mut mdb = MallocDataBlock::<i32>::new();
/// let (p, _, cap) = mdb.into_raw_parts();
/// let cap = cap as c_int;
/// let len = c_func(&mut p as *mut *mut i32 as *mut *mut void, &mut cap as *mut c_int);
/// if len > 0 {
///     let mdb = MallocDataBlock<i32>::from_raw_parts(p, len as usize, cap as usize);
///     ...
/// }
/// 
pub struct MallocDataBlock<T> {
	data: NonNull<T>,
	_marker: PhantomData<T>,
	cap: usize,
	len: usize,
}

impl <T>Default for MallocDataBlock<T> {
	fn default() -> Self { Self::new() }	
}

impl <T>Deref for MallocDataBlock<T> {
	type Target = [T];
	fn deref(&self) -> &[T] {
		unsafe { std::slice::from_raw_parts(self.data.as_ptr(), self.len) }
	}
}
impl <T>DerefMut for MallocDataBlock<T> {
	fn deref_mut(&mut self) -> &mut[T] {
		unsafe { std::slice::from_raw_parts_mut(self.data.as_ptr(), self.len) }
	}
}
impl <T> MallocDataBlock<T> {
	pub fn new() -> Self {
		Self {
			data: NonNull::dangling(),
			_marker: PhantomData,
			cap: 0,
			len: 0,
		}
	}
	pub fn into_raw_parts(self) -> (*mut T, usize, usize) {
		let mut me = core::mem::ManuallyDrop::new(self);
		(if me.cap == 0 { std::ptr::null_mut() } else { unsafe{me.data.as_mut()}}, me.len, me.cap)
	}
	
/// Creates a `MallocDataBlock<T>` directly from raw components.
///
/// # Safety
///
/// This is highly unsafe as it is assumed that
///   p has been allocated using malloc() and has size cap * sizeof(T)
/// The function will panic if p is null or len > cap
	pub unsafe fn from_raw_parts(p: *mut T, len: usize, cap: usize) -> Self {
		assert!(len <= cap);
		if cap == 0 {
			assert!(p.is_null());
			Self::new() 
		} else { Self {data: NonNull::new(p).unwrap(), _marker: PhantomData, cap, len }}
	}
}

impl <T>Drop for MallocDataBlock<T> {
	fn drop(&mut self) {
		if self.cap > 0 {
			unsafe {libc::free(self.data.as_mut() as *mut T as *mut libc::c_void)}
		}
	}
}

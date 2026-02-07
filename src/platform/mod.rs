pub mod syscall;

unsafe extern "C" {
    pub fn malloc(size: usize) -> *mut u8;
    pub fn realloc(ptr: *mut u8, size: usize) -> *mut u8;
    pub fn free(ptr: *mut u8);
    pub fn abort() -> !;
}

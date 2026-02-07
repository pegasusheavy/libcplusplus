#![no_std]

extern crate alloc;

mod platform;

#[cfg(feature = "sanitize")]
pub mod sanitize;

use core::alloc::{GlobalAlloc, Layout};

struct CAllocator;

unsafe impl GlobalAlloc for CAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        #[cfg(feature = "sanitize")]
        {
            return unsafe { sanitize::sanitized_alloc(layout) };
        }
        #[cfg(not(feature = "sanitize"))]
        {
            // SAFETY: malloc is provided by the C runtime.
            unsafe { platform::malloc(layout.size()) }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        #[cfg(feature = "sanitize")]
        {
            unsafe { sanitize::sanitized_dealloc(ptr, layout) };
            return;
        }
        #[cfg(not(feature = "sanitize"))]
        {
            // SAFETY: ptr was allocated by malloc via our alloc() above.
            unsafe { platform::free(ptr) }
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        #[cfg(feature = "sanitize")]
        {
            return unsafe { sanitize::sanitized_realloc(ptr, layout, new_size) };
        }
        #[cfg(not(feature = "sanitize"))]
        {
            // SAFETY: ptr was allocated by malloc, new_size is the requested size.
            unsafe { platform::realloc(ptr, new_size) }
        }
    }
}

#[global_allocator]
static ALLOCATOR: CAllocator = CAllocator;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // SAFETY: abort is provided by the C runtime and never returns.
    unsafe { platform::abort() }
}

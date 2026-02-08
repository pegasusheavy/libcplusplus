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
            unsafe { sanitize::sanitized_alloc(layout) }
        }
        #[cfg(not(feature = "sanitize"))]
        {
            // SAFETY: malloc is provided by the C runtime.
            unsafe { platform::malloc(layout.size()) }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, #[allow(unused_variables)] layout: Layout) {
        #[cfg(feature = "sanitize")]
        {
            unsafe { sanitize::sanitized_dealloc(ptr, layout) };
        }
        #[cfg(not(feature = "sanitize"))]
        {
            // SAFETY: ptr was allocated by malloc via our alloc() above.
            unsafe { platform::free(ptr) }
        }
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        #[allow(unused_variables)] layout: Layout,
        new_size: usize,
    ) -> *mut u8 {
        #[cfg(feature = "sanitize")]
        {
            unsafe { sanitize::sanitized_realloc(ptr, layout, new_size) }
        }
        #[cfg(not(feature = "sanitize"))]
        {
            // SAFETY: ptr was allocated by malloc, new_size is the requested size.
            unsafe { platform::realloc(ptr, new_size) }
        }
    }
}

// The test harness provides its own global allocator and panic handler.
// Only register ours for non-test builds.
#[cfg(not(test))]
#[global_allocator]
static ALLOCATOR: CAllocator = CAllocator;

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // SAFETY: abort is provided by the C runtime and never returns.
    unsafe { platform::abort() }
}

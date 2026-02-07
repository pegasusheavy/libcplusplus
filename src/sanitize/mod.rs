pub mod diagnostic;
pub mod epoch;
pub mod quarantine;
pub mod redzone;
pub mod spinlock;
pub mod tracker;

use core::alloc::Layout;
use tracker::AllocKind;

/// Sanitized allocation: adds redzones, tracks the allocation.
///
/// # Safety
/// Must be called from a GlobalAlloc implementation. The returned pointer
/// is offset by REDZONE_SIZE from the real malloc'd base.
pub unsafe fn sanitized_alloc(layout: Layout) -> *mut u8 {
    let user_size = layout.size();
    let total = redzone::total_size(user_size);

    // SAFETY: malloc is provided by the C runtime.
    let base = unsafe { crate::platform::malloc(total) };
    if base.is_null() {
        return core::ptr::null_mut();
    }

    // SAFETY: base points to `total` bytes of writable memory.
    unsafe { redzone::fill_canaries(base, user_size) };

    let user_ptr = unsafe { base.add(redzone::REDZONE_SIZE) };

    tracker::insert(user_ptr as usize, user_size, AllocKind::Rust);

    user_ptr
}

/// Sanitized deallocation: checks for errors, quarantines the block.
///
/// # Safety
/// `ptr` must have been returned by `sanitized_alloc`.
pub unsafe fn sanitized_dealloc(ptr: *mut u8, _layout: Layout) {
    dealloc_inner(ptr, AllocKind::Rust);
}

/// Deallocation logic shared between the global allocator and future
/// operator delete exports. `expected_kind` is checked against the
/// tracked allocation kind.
pub fn dealloc_inner(ptr: *mut u8, expected_kind: AllocKind) {
    if ptr.is_null() {
        return;
    }

    let user_addr = ptr as usize;

    match tracker::remove(user_addr) {
        Some((tracked_size, tracked_kind)) => {
            // Check alloc/dealloc kind matches (new vs new[], etc.).
            if !kind_compatible(tracked_kind, expected_kind) {
                diagnostic::mismatched_dealloc(user_addr, tracked_kind, expected_kind);
            }

            // Check redzones for overflow/underflow.
            let base = unsafe { ptr.sub(redzone::REDZONE_SIZE) };
            // SAFETY: base is the original malloc'd pointer.
            unsafe { redzone::check_canaries(base, tracked_size, user_addr) };

            // Poison user data to catch use-after-free reads.
            // SAFETY: ptr points to tracked_size bytes of allocated memory.
            unsafe { redzone::poison(ptr, tracked_size) };

            // Quarantine instead of immediately freeing.
            let evicted = quarantine::push(user_addr, base as usize, tracked_size);

            // If the quarantine evicted an old entry, actually free it now.
            if let Some(base_addr) = evicted {
                // SAFETY: base_addr was previously returned by malloc.
                unsafe { crate::platform::free(base_addr as *mut u8) };
            }
        }
        None => {
            if quarantine::contains(user_addr) {
                diagnostic::double_free(user_addr);
            } else {
                diagnostic::invalid_free(user_addr);
            }
        }
    }
}

/// Sanitized reallocation: alloc + copy + dealloc.
///
/// Cannot use platform realloc directly because of redzone layout.
///
/// # Safety
/// `ptr` must have been returned by `sanitized_alloc`. `new_size` must be > 0.
pub unsafe fn sanitized_realloc(ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
    let new_ptr = unsafe { sanitized_alloc(new_layout) };
    if new_ptr.is_null() {
        return new_ptr;
    }

    let copy_size = if layout.size() < new_size {
        layout.size()
    } else {
        new_size
    };
    // SAFETY: Both pointers are valid for copy_size bytes, non-overlapping
    // (sanitized_alloc returned a fresh allocation).
    unsafe { core::ptr::copy_nonoverlapping(ptr, new_ptr, copy_size) };

    // Free old block through sanitized path.
    unsafe { sanitized_dealloc(ptr, layout) };

    new_ptr
}

/// Rust alloc is compatible with itself. Future operator new/delete
/// will enforce scalar vs array matching.
fn kind_compatible(tracked: AllocKind, freed: AllocKind) -> bool {
    match (tracked, freed) {
        (AllocKind::Rust, AllocKind::Rust) => true,
        (AllocKind::ScalarNew, AllocKind::ScalarNew) => true,
        (AllocKind::ArrayNew, AllocKind::ArrayNew) => true,
        _ => false,
    }
}

/// Size of each red zone (prefix and suffix), in bytes.
/// 16 bytes aligns with malloc's max_align_t guarantee on x86_64.
pub const REDZONE_SIZE: usize = 16;

/// Byte pattern written into red zones.
pub const CANARY_BYTE: u8 = 0xAB;

/// Byte pattern written over freed user data.
pub const POISON_BYTE: u8 = 0xFE;

/// Total allocation size including both redzones.
pub const fn total_size(user_size: usize) -> usize {
    REDZONE_SIZE + user_size + REDZONE_SIZE
}

/// Fill prefix and suffix redzones with canary bytes.
///
/// `base` points to the start of the malloc'd block.
/// The user region starts at `base + REDZONE_SIZE`.
///
/// # Safety
/// `base` must point to at least `total_size(user_size)` writable bytes.
pub unsafe fn fill_canaries(base: *mut u8, user_size: usize) {
    // Prefix redzone.
    // SAFETY: base is valid for REDZONE_SIZE bytes (caller guarantees total_size).
    unsafe { core::ptr::write_bytes(base, CANARY_BYTE, REDZONE_SIZE) };

    // Suffix redzone.
    // SAFETY: base + REDZONE_SIZE + user_size is still within the allocation.
    let suffix = unsafe { base.add(REDZONE_SIZE + user_size) };
    unsafe { core::ptr::write_bytes(suffix, CANARY_BYTE, REDZONE_SIZE) };
}

/// Check that redzones are intact.
///
/// On corruption, calls `diagnostic::overflow_detected` which aborts.
///
/// # Safety
/// `base` must point to the start of the malloc'd block with valid redzones.
pub unsafe fn check_canaries(base: *mut u8, user_size: usize, user_addr: usize) {
    let mut prefix_corrupt = false;
    let mut suffix_corrupt = false;

    // Check prefix.
    for i in 0..REDZONE_SIZE {
        // SAFETY: i < REDZONE_SIZE, within the allocation.
        if unsafe { *base.add(i) } != CANARY_BYTE {
            prefix_corrupt = true;
            break;
        }
    }

    // Check suffix.
    // SAFETY: suffix starts at base + REDZONE_SIZE + user_size, within the allocation.
    let suffix = unsafe { base.add(REDZONE_SIZE + user_size) };
    for i in 0..REDZONE_SIZE {
        if unsafe { *suffix.add(i) } != CANARY_BYTE {
            suffix_corrupt = true;
            break;
        }
    }

    if prefix_corrupt || suffix_corrupt {
        crate::sanitize::diagnostic::overflow_detected(
            user_addr,
            user_size,
            prefix_corrupt,
            suffix_corrupt,
        );
    }
}

/// Poison the user region with a recognizable pattern to catch use-after-free reads.
///
/// # Safety
/// `user_ptr` must point to at least `user_size` writable bytes.
pub unsafe fn poison(user_ptr: *mut u8, user_size: usize) {
    // SAFETY: user_ptr is valid for user_size bytes (the original allocation).
    unsafe { core::ptr::write_bytes(user_ptr, POISON_BYTE, user_size) };
}

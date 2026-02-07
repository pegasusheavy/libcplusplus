/// Raw Linux x86_64 syscall wrappers.
///
/// These bypass all Rust and C library layers, issuing syscalls directly
/// via the `syscall` instruction.

/// Write bytes to a file descriptor.
/// Returns the number of bytes written, or a negative errno on failure.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[inline(always)]
pub unsafe fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let ret: isize;
    // SAFETY: Caller guarantees buf points to len readable bytes.
    // The syscall instruction is the only way to invoke the kernel.
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") 1_isize => ret,
            in("rdi") fd,
            in("rsi") buf,
            in("rdx") len,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

/// Terminate the calling process and all its threads.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[inline(always)]
pub unsafe fn sys_exit_group(code: i32) -> ! {
    // SAFETY: This terminates the process. Caller is responsible for
    // ensuring this is the intended behavior.
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") 231_usize,
            in("rdi") code as usize,
            options(noreturn, nostack),
        );
    }
}

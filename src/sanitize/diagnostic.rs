use crate::sanitize::tracker::AllocKind;

const HEADER: &[u8] = b"\n\x1b[1;31m=== libcplusplus sanitizer ===\x1b[0m\n";

pub fn write_stderr(msg: &[u8]) {
    // SAFETY: sys_write to fd 2 (stderr) is always valid.
    unsafe { crate::platform::syscall::sys_write(2, msg.as_ptr(), msg.len()) };
}

/// Format a usize as a 16-digit zero-padded hex string with 0x prefix.
/// Writes into the provided 18-byte buffer and returns a slice of it.
pub fn format_hex(value: usize, buf: &mut [u8; 18]) -> &[u8] {
    buf[0] = b'0';
    buf[1] = b'x';
    let mut v = value;
    for i in (2..18).rev() {
        let digit = (v & 0xF) as u8;
        buf[i] = if digit < 10 {
            b'0' + digit
        } else {
            b'a' + digit - 10
        };
        v >>= 4;
    }
    buf
}

/// Format a usize as a variable-length decimal string.
/// Writes right-aligned into the provided 20-byte buffer and returns
/// the populated slice.
pub fn format_dec(value: usize, buf: &mut [u8; 20]) -> &[u8] {
    if value == 0 {
        buf[19] = b'0';
        return &buf[19..];
    }
    let mut v = value;
    let mut i = 20;
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    &buf[i..]
}

fn kind_name(kind: AllocKind) -> &'static [u8] {
    match kind {
        AllocKind::Rust => b"rust alloc",
        AllocKind::ScalarNew => b"operator new",
        AllocKind::ArrayNew => b"operator new[]",
    }
}

fn report_abort() -> ! {
    write_stderr(b"aborting.\n\n");
    // SAFETY: abort is provided by the C runtime.
    unsafe { crate::platform::abort() }
}

// --- Error reporters ---
// Each prints a diagnostic to stderr and aborts.

pub fn double_free(addr: usize) -> ! {
    write_stderr(HEADER);
    write_stderr(b"ERROR: double-free\n");
    write_stderr(b"  address: ");
    let mut buf = [0u8; 18];
    write_stderr(format_hex(addr, &mut buf));
    write_stderr(b"\n  This address was already freed and is still in quarantine.\n");
    report_abort();
}

pub fn invalid_free(addr: usize) -> ! {
    write_stderr(HEADER);
    write_stderr(b"ERROR: invalid free\n");
    write_stderr(b"  address: ");
    let mut buf = [0u8; 18];
    write_stderr(format_hex(addr, &mut buf));
    write_stderr(b"\n  This address was not returned by any tracked allocation.\n");
    report_abort();
}

pub fn mismatched_dealloc(addr: usize, expected: AllocKind, actual: AllocKind) -> ! {
    write_stderr(HEADER);
    write_stderr(b"ERROR: mismatched deallocation\n");
    write_stderr(b"  address:        ");
    let mut buf = [0u8; 18];
    write_stderr(format_hex(addr, &mut buf));
    write_stderr(b"\n  allocated with: ");
    write_stderr(kind_name(expected));
    write_stderr(b"\n  freed with:     ");
    write_stderr(kind_name(actual));
    write_stderr(b"\n");
    report_abort();
}

pub fn overflow_detected(
    addr: usize,
    size: usize,
    prefix_corrupt: bool,
    suffix_corrupt: bool,
) -> ! {
    write_stderr(HEADER);
    write_stderr(b"ERROR: buffer overflow detected (red zone corruption)\n");
    write_stderr(b"  address: ");
    let mut hex_buf = [0u8; 18];
    write_stderr(format_hex(addr, &mut hex_buf));
    write_stderr(b"\n  size:    ");
    let mut dec_buf = [0u8; 20];
    write_stderr(format_dec(size, &mut dec_buf));
    write_stderr(b" bytes\n");
    if prefix_corrupt {
        write_stderr(b"  -> underflow: prefix red zone corrupted\n");
    }
    if suffix_corrupt {
        write_stderr(b"  -> overflow: suffix red zone corrupted\n");
    }
    report_abort();
}

pub fn leak_detected(addr: usize, size: usize, kind: AllocKind) {
    write_stderr(b"  LEAK: ");
    let mut hex_buf = [0u8; 18];
    write_stderr(format_hex(addr, &mut hex_buf));
    write_stderr(b"  size=");
    let mut dec_buf = [0u8; 20];
    write_stderr(format_dec(size, &mut dec_buf));
    write_stderr(b"  via=");
    write_stderr(kind_name(kind));
    write_stderr(b"\n");
}

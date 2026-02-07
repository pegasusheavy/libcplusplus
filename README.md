# libcplusplus

A reimplementation of LLVM's [libc++](https://libcxx.llvm.org/) in Rust.

The long-term goal is a fully conformant, ABI-compatible drop-in replacement
for libc++ that can be linked into C++ programs via standard toolchains, backed
by Rust's safety guarantees. The output artifact is a `cdylib` / `staticlib`
that a C++ toolchain links against in place of (or alongside) the real libc++.

## Status

Early development. The project scaffolding, platform abstraction layer, and
sanitizer infrastructure are in place. Core library components (allocator,
strings, containers, smart pointers, I/O, exceptions) are planned but not yet
implemented. See [TODO.md](TODO.md) for the full task breakdown and
[AGENTS.md](AGENTS.md) for the architecture and design principles.

## Design

- **Fully `#![no_std]`** — only `core` and `alloc`. No libc crate. I/O via
  raw Linux syscalls (`core::arch::asm!`). Synchronization via
  `core::sync::atomic`.
- **ABI-first** — exported symbols match the Itanium C++ ABI mangled names
  that clang/libc++ emits. `#[repr(C)]` structs match libc++ memory layout
  field-for-field.
- **Zero external dependencies** — no crates.io deps. Only platform-provided
  `malloc`/`free`/`realloc`/`abort` and Linux syscalls.
- **Built-in memory sanitizer** — `cargo build --features sanitize` enables
  red zones, allocation tracking, freed-memory quarantine, and diagnostics
  for double-free, invalid free, mismatched new/delete, and buffer overflow.

## Building

Requires Rust 2024 edition (1.85+). Targets x86_64 Linux.

```bash
# Default build (zero overhead, no sanitizer)
cargo build

# With memory sanitizer enabled
cargo build --features sanitize

# Release build
cargo build --release
```

## Sanitizer

When built with `--features sanitize`, every allocation is instrumented:

- **Red zones** — 16-byte canary regions before and after each allocation
  detect buffer overflow and underflow on deallocation.
- **Allocation tracker** — a 16K-entry hash table tracks all live allocations
  to catch double-free and invalid free.
- **Quarantine** — freed blocks are held in a 256-entry ring buffer with
  poisoned memory (`0xFE`) to surface use-after-free.
- **Mismatch detection** — scalar `new` freed with array `delete[]` (and vice
  versa) is caught when operator new/delete exports are wired up.
- **Leak reporting** — all unfreed allocations can be dumped at exit.

Diagnostics are printed to stderr and the process aborts. No runtime overhead
when the feature is disabled.

## Project Structure

```
src/
├── lib.rs              # Crate root, global allocator, panic handler
├── platform/           # malloc/free FFI, raw Linux syscall wrappers
└── sanitize/           # Feature-gated memory sanitizer
    ├── tracker.rs      # Live allocation hash table
    ├── quarantine.rs   # Freed-block ring buffer
    ├── redzone.rs      # Canary byte overflow detection
    ├── diagnostic.rs   # Stderr error reporting
    ├── spinlock.rs     # Minimal TTAS spin lock
    └── epoch.rs        # Generation counter for iterator invalidation
```

See [AGENTS.md](AGENTS.md) for the full planned architecture including
allocator, strings, containers, smart pointers, I/O, exceptions, RTTI, and
ABI helpers.

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

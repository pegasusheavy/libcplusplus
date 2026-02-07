# AGENTS.md — libcplusplus

## Project Overview

**libcplusplus** is a reimplementation of LLVM's libc++ (the C++ standard library) in Rust. The long-term goal is **full conformance** with the C++ standard library specification — a complete, ABI-compatible drop-in replacement for libc++ backed by Rust's safety guarantees. Development starts as a PoC covering core components and incrementally expands toward full coverage.

The output artifact is a `cdylib` / `staticlib` that a C++ toolchain can link against in place of (or alongside) the real libc++.

## `no_std` Constraints

This crate is **unconditionally `#![no_std]`**. This means:

- **No `std` imports anywhere.** Use `core::*` and `alloc::*` only.
- **Custom global allocator required.** The crate defines a `#[global_allocator]` that calls `malloc`/`free`/`realloc` via `extern "C"` — these symbols are provided by the C runtime the final binary links against. No `libc` crate.
- **Panic handler required.** The crate provides `#[panic_handler]` that calls `abort()` via `extern "C"`. In `cdylib`/`staticlib` mode, Rust does not provide one.
- **No `println!`, `format!`, `String` from std.** Use `alloc::string::String`, `alloc::vec::Vec`, `alloc::format!`, or manual formatting into stack buffers.
- **I/O via raw syscalls.** `core::arch::asm!` with the Linux `write` syscall number (`1` on x86_64). No `std::io`, no `libc::write`.
- **Synchronization via atomics.** `core::sync::atomic` for all concurrency. No `std::sync::Mutex` or `std::sync::Once`. Spin-locks or Linux futex syscalls where needed.

## Architecture

```
libcplusplus/
├── Cargo.toml
├── AGENTS.md
├── src/
│   ├── lib.rs              # Crate root: #![no_std], extern crate alloc, panic handler, global allocator
│   ├── platform/
│   │   ├── mod.rs           # Platform abstraction — raw syscall wrappers, malloc/free FFI
│   │   └── syscall.rs       # Linux x86_64 syscall helpers (write, futex, exit_group)
│   ├── sanitize/            # (feature-gated behind `sanitize`)
│   │   ├── mod.rs           # Sanitized alloc/dealloc/realloc entry points
│   │   ├── spinlock.rs      # Minimal spin lock (TTAS) for sanitizer internals
│   │   ├── tracker.rs       # Fixed-capacity open-addressing hash table of live allocations
│   │   ├── quarantine.rs    # Ring buffer of recently-freed blocks (use-after-free detection)
│   │   ├── redzone.rs       # Canary bytes before/after allocations (overflow detection)
│   │   ├── diagnostic.rs    # Stderr error reporter + hex/decimal formatting
│   │   └── epoch.rs         # Generation counter for iterator invalidation
│   ├── allocator/
│   │   ├── mod.rs           # C++ operator new / operator delete backed by Rust's GlobalAlloc
│   │   └── aligned.rs       # Aligned allocation variants (new(std::align_val_t))
│   ├── strings/
│   │   ├── mod.rs           # std::string (basic_string<char>) — SSO buffer, ABI layout
│   │   ├── sso.rs           # Short-string-optimization repr (union of inline/heap)
│   │   └── traits.rs        # char_traits<char> function table
│   ├── containers/
│   │   ├── mod.rs
│   │   ├── vector.rs        # std::vector<T> — contiguous growable array
│   │   └── unique_ptr.rs    # std::unique_ptr<T> — RAII pointer, maps to Box<T>
│   ├── io/
│   │   ├── mod.rs
│   │   ├── ostream.rs       # Minimal std::ostream / cout stub (writes via raw syscall)
│   │   └── formatting.rs    # Numeric and string formatting into stack buffers
│   ├── memory/
│   │   ├── mod.rs
│   │   ├── shared_ptr.rs    # std::shared_ptr<T> — reference-counted pointer (Arc analog)
│   │   └── weak_ptr.rs      # std::weak_ptr<T>
│   ├── exception/
│   │   ├── mod.rs           # __cxa_throw, __cxa_allocate_exception stubs
│   │   └── unwind.rs        # Itanium ABI _Unwind_* glue (calls into libunwind)
│   ├── typeinfo/
│   │   ├── mod.rs           # std::type_info, typeid() vtable layout
│   │   └── rtti.rs          # __cxxabiv1 class type info structs
│   └── abi/
│       ├── mod.rs           # Itanium C++ ABI helpers (__cxa_guard_*, __cxa_atexit)
│       └── guard.rs         # Static-local initialization guards (atomics + futex)
└── tests/
    ├── link_test.cpp        # Minimal C++ program that links against the Rust lib
    └── build.rs             # cc crate build script for compiling the C++ test
```

## Design Principles

1. **ABI-first.** Every exported symbol must match the mangled name and calling convention that clang/libc++ emits. Use `#[no_mangle]`, `#[export_name = "..."]`, and `extern "C"` to control this precisely. Check against `nm -D` on the real libc++.so.

2. **Fully `#![no_std]`.** The entire crate is `#![no_std]`. No module may depend on `std`. Only `core` and `alloc` are available. The crate provides its own global allocator backed by the platform's `malloc`/`free` via raw `extern "C"` FFI — no `libc` crate dependency. I/O uses inline syscalls (`syscall(SYS_write, ...)` via `core::arch::asm!`), not any Rust std or libc wrappers.

3. **Unsafe at the boundary, safe inside.** Every `extern "C"` function is inherently unsafe to call from C++, but the Rust implementation behind it should be safe Rust wherever possible. Isolate `unsafe` blocks to the FFI shim layer.

4. **Match libc++ memory layout exactly.** For types like `std::string` and `std::vector`, the Rust struct must be `#[repr(C)]` and field-for-field identical to the libc++ layout on the target platform (x86_64 Linux, Itanium ABI). Verify with `static_assert(sizeof(...))` in the C++ test harness.

5. **Full conformance is the goal.** Development starts with a PoC covering the most-used components, but the trajectory is toward full C++ standard library conformance. Early phases focus on the happy path and common concrete types (`char`, `int`, `void*`), but each module should be designed with extensibility toward complete coverage. Panics are acceptable as temporary placeholders where C++ would throw, to be replaced with proper exception-handling ABI support later.

6. **Zero external dependencies.** No crates.io dependencies. The only "foreign" functions are platform-provided: `malloc`, `free`, `realloc`, `abort` (linked from the C runtime that every C++ program already provides), and Linux syscalls via `core::arch::asm!`.

7. **`sanitize` feature flag for memory safety diagnostics.** `cargo build --features sanitize` enables a built-in memory sanitizer. When enabled, the global allocator wraps every allocation with red zones, tracks all live allocations in a static hash table, quarantines freed blocks to catch use-after-free, and reports errors (double-free, invalid free, mismatched new/delete, buffer overflow) to stderr before aborting. Compiles to zero overhead when disabled. Future container modules use epoch counters to detect iterator invalidation.

## Implementation Phases

### Phase 1 — Foundation

- Set up `Cargo.toml` with `crate-type = ["cdylib", "staticlib"]`, no dependencies.
- Set up `lib.rs` with `#![no_std]`, `extern crate alloc`, `#[panic_handler]`, and `#[global_allocator]`.
- Implement `platform/` module: raw `extern "C"` bindings for `malloc`/`free`/`realloc`/`abort`, and `core::arch::asm!` wrappers for `SYS_write`, `SYS_futex`, `SYS_exit_group`.
- Implement `operator new` / `operator delete` in `allocator/` that forward to Rust's global allocator.
- Implement `__cxa_guard_acquire` / `__cxa_guard_release` using `core::sync::atomic` (AtomicU8 + spin/futex, no `std::sync::Once`).
- Implement `__cxa_atexit` registration (fixed-capacity static array of function pointers, no heap).
- Write a C++ test that calls `new` / `delete` and links against the Rust library.

### Phase 2 — Strings

- Define `#[repr(C)]` struct matching libc++ `basic_string<char>` layout (with SSO).
- Implement construction, destruction, `c_str()`, `size()`, `operator[]`.
- Export mangled symbol names: `_ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE...`.
- Validate by linking a C++ program that constructs and prints a `std::string`.

### Phase 3 — Containers

- `std::vector<int>` — `#[repr(C)]` triple-pointer layout (`begin`, `end`, `end_cap`).
- `std::unique_ptr<T>` — thin wrapper; essentially `Box<T>` with C++ ABI.
- Export constructors, destructors, `push_back`, `size`, `operator[]`.

### Phase 4 — Smart Pointers

- `std::shared_ptr<T>` — control block with strong/weak counts, backed by `Arc`-like logic.
- `std::weak_ptr<T>` — weak reference with `lock()`.
- Must match libc++ control block layout for ABI compat.

### Phase 5 — I/O Stubs

- Minimal `std::cout` that writes to fd 1 via inline `syscall(SYS_write, 1, buf, len)` using `core::arch::asm!`.
- `operator<<` for `const char*`, `int`, `std::string`.
- Integer-to-string formatting done in a stack buffer using `core` math — no `format!` or `alloc::string`.
- Enough to make `std::cout << "hello" << std::endl;` work.

### Phase 6 — Exception Handling Stubs

- `__cxa_allocate_exception`, `__cxa_throw` — allocate and "throw" (currently: abort with message).
- `__cxa_begin_catch`, `__cxa_end_catch` — no-op stubs for linking.
- Real unwinding deferred; PoC focuses on the non-exceptional path.

## Build & Test

```bash
# Build the Rust library
cargo build

# Run Rust-side unit tests
cargo test

# Build and run the C++ link test (once tests/build.rs is set up)
cargo test --test link_test
```

## Key References

- [libc++ source (LLVM)](https://github.com/llvm/llvm-project/tree/main/libcxx) — the canonical implementation to match.
- [Itanium C++ ABI](https://itanium-cxx-abi.github.io/cxx-abi/abi.html) — name mangling, vtable layout, exception handling.
- [libc++abi source](https://github.com/llvm/llvm-project/tree/main/libcxxabi) — `__cxa_*` function specs.
- Rust `#[repr(C)]` and FFI: https://doc.rust-lang.org/nomicon/ffi.html
- Linux syscall table (x86_64): https://filippo.io/linux-syscall-table/

## Conventions

- All exported C++ symbols go through `#[export_name]` or `#[no_mangle]`. Never rely on Rust's default mangling.
- Use `// ABI: <description>` comments on any struct field or function whose layout/signature is dictated by the C++ ABI.
- Every `unsafe` block must have a `// SAFETY: ...` comment explaining the invariant.
- Platform-specific code gated behind `#[cfg(target_os = "linux")]` and `#[cfg(target_arch = "x86_64")]`. This PoC targets x86_64 Linux only.
- Never import or depend on `std`. The crate is `#![no_std]` unconditionally. If you need `Vec`, `Box`, `String`, or `Arc`, import from `alloc`. If you need `HashMap`, implement a minimal one or use a different data structure.

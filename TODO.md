# TODO — libcplusplus PoC

## Phase 0 — Scaffolding (DONE)

- [x] `Cargo.toml` — `crate-type = ["cdylib", "staticlib"]`, `panic = "abort"`, `sanitize` feature flag
- [x] `lib.rs` — `#![no_std]`, `extern crate alloc`, `#[panic_handler]` (abort), `#[global_allocator]` (malloc/free)
- [x] `platform/mod.rs` — `unsafe extern "C"` bindings for `malloc`, `free`, `realloc`, `abort`
- [x] `platform/syscall.rs` — `sys_write`, `sys_exit_group` via `core::arch::asm!`
- [x] `sanitize/` module — full sanitizer infrastructure (see Sanitize section below)

## Sanitize Feature (DONE)

### Infrastructure (`src/sanitize/`)
- [x] `spinlock.rs` — TTAS spin lock with `SpinLockGuard` (RAII)
- [x] `tracker.rs` — 16384-entry open-addressing hash table (Fibonacci hashing, tombstone deletion)
  - [x] `insert(addr, size, kind)`
  - [x] `remove(addr)` — returns `(size, AllocKind)` or `None`
  - [x] `lookup(addr)`
  - [x] `report_leaks()` — walks all live entries
- [x] `quarantine.rs` — 256-entry ring buffer of recently-freed blocks
  - [x] `push(user_addr, base_addr, user_size)` — returns evicted base for actual free
  - [x] `contains(user_addr)` — linear scan for double-free detection
- [x] `redzone.rs` — 16-byte canary zones before/after each allocation
  - [x] `fill_canaries(base, user_size)`
  - [x] `check_canaries(base, user_size, user_addr)` — aborts on corruption
  - [x] `poison(user_ptr, user_size)` — fills freed data with `0xFE`
- [x] `diagnostic.rs` — stderr output via `sys_write(2, ...)`
  - [x] `format_hex` / `format_dec` — no_std number formatting
  - [x] `double_free`, `invalid_free`, `mismatched_dealloc`, `overflow_detected`, `leak_detected`
- [x] `epoch.rs` — `AtomicU64` generation counter for future iterator invalidation
- [x] `mod.rs` — `sanitized_alloc`, `sanitized_dealloc`, `sanitized_realloc`, `dealloc_inner`
  - [x] Alloc: malloc(redzone + size + redzone), fill canaries, track
  - [x] Dealloc: verify tracked, check kind match, check canaries, poison, quarantine
  - [x] Realloc: alloc + copy + dealloc (can't use platform realloc with redzones)
- [x] CAllocator in `lib.rs` — dispatches to sanitized path when feature enabled

### Future Sanitize Work
- [ ] Hook `operator new`/`delete` exports into sanitizer (with `AllocKind::ScalarNew`/`ArrayNew`)
- [ ] Bounds checking on `vector::operator[]` and `string::operator[]`
- [ ] Epoch-based iterator invalidation in containers (bump on mutate, check on deref)
- [ ] Null-dereference detection on `unique_ptr`/`shared_ptr` `operator*`/`operator->`
- [ ] Use-after-move detection on `unique_ptr`
- [ ] Leak report on `__cxa_finalize` (call `tracker::report_leaks()`)
- [ ] `sys_futex` for contended spin lock fallback (currently pure spin)

## Phase 1 — Foundation

### Platform Layer (`src/platform/`)
- [x] Create `platform/mod.rs` — re-export submodules, `unsafe extern "C"` bindings for `malloc`, `free`, `realloc`, `abort`
- [x] Create `platform/syscall.rs` — `core::arch::asm!` wrappers for x86_64 Linux:
  - [x] `sys_write(fd, buf, len) -> isize`
  - [ ] `sys_futex(addr, op, val, ...) -> isize`
  - [x] `sys_exit_group(code) -> !`

### Allocator (`src/allocator/`)
- [ ] Create `allocator/mod.rs` — export `operator new` / `operator delete` variants:
  - [ ] `_Znwm` — `operator new(size_t)`
  - [ ] `_ZdlPv` — `operator delete(void*)`
  - [ ] `_Znam` — `operator new[](size_t)`
  - [ ] `_ZdaPv` — `operator delete[](void*)`
  - [ ] `_ZdlPvm` — `operator delete(void*, size_t)` (C++14 sized deallocation)
  - [ ] `_ZdaPvm` — `operator delete[](void*, size_t)`
- [ ] Create `allocator/aligned.rs` — aligned `new`/`delete`:
  - [ ] `_ZnwmSt11align_val_t` — `operator new(size_t, align_val_t)`
  - [ ] `_ZdlPvSt11align_val_t` — `operator delete(void*, align_val_t)`
  - [ ] Implement via `posix_memalign` or manual alignment on top of `malloc`
- [ ] Verify symbols match `nm -D /usr/lib/libc++.so.1 | grep '_Znw\|_Zdl\|_Zna\|_Zda'`

### ABI Helpers (`src/abi/`)
- [ ] Create `abi/mod.rs`
- [ ] Create `abi/guard.rs` — static-local init guards:
  - [ ] `__cxa_guard_acquire(guard: *mut u64) -> i32`
  - [ ] `__cxa_guard_release(guard: *mut u64)`
  - [ ] `__cxa_guard_abort(guard: *mut u64)`
  - [ ] Use `AtomicU8` for guard byte, futex for contention
- [ ] Implement `__cxa_atexit(fn, arg, dso_handle)` — fixed-capacity static table
- [ ] Implement `__cxa_finalize(dso_handle)` — run registered atexit handlers

### C++ Link Test
- [ ] Create `tests/link_test.cpp` — minimal C++ program:
  - [ ] Calls `new` / `delete`
  - [ ] Uses a static-local variable (triggers guard functions)
- [ ] Create build script to compile and link test against the Rust staticlib
- [ ] Verify clean link with no unresolved symbols for tested features

## Phase 2 — Strings

### Reverse-Engineer Layout
- [ ] Dump libc++ `basic_string<char>` layout: `sizeof`, `offsetof` each field
- [ ] Document SSO threshold (typically 22 bytes on 64-bit)
- [ ] Document short-mode vs long-mode flag bit positions

### Implementation (`src/strings/`)
- [ ] Create `strings/mod.rs` — re-exports, top-level FFI entry points
- [ ] Create `strings/sso.rs` — `#[repr(C)]` struct:
  - [ ] Short string representation (inline buffer + size in last byte)
  - [ ] Long string representation (pointer + size + capacity)
  - [ ] `is_long()` / `set_short_size()` / `set_long_size()`
- [ ] Create `strings/traits.rs` — `char_traits<char>` functions:
  - [ ] `length(s)` — `strlen` equivalent
  - [ ] `copy(dst, src, n)` — `memcpy` equivalent
  - [ ] `compare(a, b, n)` — `memcmp` equivalent
  - [ ] `assign(dst, c)` / `assign(dst, n, c)` — `memset` equivalent
- [ ] Export mangled symbols for `basic_string<char>`:
  - [ ] Default constructor
  - [ ] Constructor from `const char*`
  - [ ] Constructor from `const char*` + `size_t`
  - [ ] Copy constructor
  - [ ] Move constructor
  - [ ] Destructor
  - [ ] `c_str()` / `data()`
  - [ ] `size()` / `length()`
  - [ ] `operator[]`
  - [ ] `append(const char*, size_t)`
- [ ] Add `static_assert` checks in C++ test harness for `sizeof` / `alignof`

### Validation
- [ ] C++ test: construct a `std::string` from a literal, read back via `c_str()`, verify contents
- [ ] C++ test: construct a string longer than SSO threshold, verify heap allocation path

## Phase 3 — Containers

### `std::vector<int>` (`src/containers/vector.rs`)
- [ ] Reverse-engineer libc++ `vector<int>` layout: `__begin`, `__end`, `__end_cap`
- [ ] Define `#[repr(C)]` struct with three `*mut T` fields
- [ ] Export mangled symbols:
  - [ ] Default constructor
  - [ ] Destructor
  - [ ] `push_back(const int&)`
  - [ ] `size()`
  - [ ] `capacity()`
  - [ ] `operator[](size_t)`
  - [ ] `data()`
- [ ] Implement growth strategy (typically 2x, matching libc++ behavior)
- [ ] C++ test: push values, read back, verify ordering and contents

### `std::unique_ptr<void>` (`src/containers/unique_ptr.rs`)
- [ ] Verify libc++ layout (single pointer, no overhead for default deleter)
- [ ] Define `#[repr(C)]` struct wrapping `*mut T`
- [ ] Export mangled symbols:
  - [ ] Constructor from raw pointer
  - [ ] Move constructor
  - [ ] Destructor (calls `operator delete`)
  - [ ] `get()`
  - [ ] `release()`
  - [ ] `reset(ptr)`
- [ ] C++ test: construct, move, verify `get()`, verify destruction calls `delete`

## Phase 4 — Smart Pointers

### `std::shared_ptr<void>` (`src/memory/shared_ptr.rs`)
- [ ] Reverse-engineer libc++ `shared_ptr` layout:
  - [ ] Object pointer + control block pointer
  - [ ] Control block: vtable ptr, strong count, weak count
- [ ] Define `#[repr(C)]` structs for both shared_ptr and control block
- [ ] Implement atomic reference counting via `core::sync::atomic`:
  - [ ] `add_shared()` — `AtomicUsize::fetch_add(1, Acquire)`
  - [ ] `release_shared()` — `fetch_sub(1, Release)`, call destructor + dealloc at zero
- [ ] Export mangled symbols:
  - [ ] Constructor from raw pointer
  - [ ] Copy constructor (bump strong count)
  - [ ] Move constructor (transfer ownership)
  - [ ] Destructor (decrement strong count)
  - [ ] `get()`
  - [ ] `use_count()`

### `std::weak_ptr<void>` (`src/memory/weak_ptr.rs`)
- [ ] Define `#[repr(C)]` struct (same dual-pointer layout)
- [ ] Implement `lock()` — atomic CAS loop on strong count
- [ ] Export mangled symbols:
  - [ ] Constructor from `shared_ptr`
  - [ ] Copy / move constructors
  - [ ] Destructor (decrement weak count, dealloc control block at zero)
  - [ ] `lock()` — returns `shared_ptr`
  - [ ] `expired()`
- [ ] C++ test: create shared_ptr, take weak_ptr, drop shared_ptr, verify `expired()`

## Phase 5 — I/O Stubs

### Syscall I/O (`src/io/`)
- [ ] Create `io/mod.rs`
- [ ] Create `io/formatting.rs` — stack-buffer formatters:
  - [ ] `format_int(value: i64, buf: &mut [u8]) -> &[u8]` — decimal
  - [ ] `format_uint(value: u64, buf: &mut [u8]) -> &[u8]` — decimal
- [ ] Create `io/ostream.rs`:
  - [ ] Define `#[repr(C)]` ostream stub (minimal fields to satisfy ABI)
  - [ ] Implement `write(buf, len)` — calls `sys_write(1, buf, len)`
  - [ ] Export `operator<<` mangled symbols for:
    - [ ] `const char*`
    - [ ] `int` / `long`
    - [ ] `basic_string<char>` (reads from SSO struct)
  - [ ] Export `std::endl` — writes `\n` + flush (no-op flush for PoC)
- [ ] Define global `cout` symbol (static ostream instance)
- [ ] C++ test: `std::cout << "hello " << 42 << std::endl;` — verify output to stdout

## Phase 6 — Exception Handling Stubs

### Exception ABI (`src/exception/`)
- [ ] Create `exception/mod.rs`
- [ ] Implement `__cxa_allocate_exception(size: usize) -> *mut u8` — malloc wrapper
- [ ] Implement `__cxa_free_exception(ptr: *mut u8)` — free wrapper
- [ ] Implement `__cxa_throw(exception, tinfo, destructor) -> !` — abort with diagnostic (write to stderr via `sys_write(2, ...)`)
- [ ] Implement no-op stubs for linking:
  - [ ] `__cxa_begin_catch(exception) -> *mut u8`
  - [ ] `__cxa_end_catch()`
  - [ ] `__cxa_rethrow() -> !`
  - [ ] `__cxa_current_exception_type() -> *const TypeInfo`

### Unwind Stubs (`src/exception/unwind.rs`)
- [ ] Export `_Unwind_Resume(exception) -> !` — abort
- [ ] Export `_Unwind_RaiseException` — abort
- [ ] Export `_Unwind_DeleteException` — no-op / free

### Type Info (`src/typeinfo/`)
- [ ] Create `typeinfo/mod.rs`
- [ ] Create `typeinfo/rtti.rs`:
  - [ ] Define `#[repr(C)]` `type_info` struct (vtable ptr + name ptr)
  - [ ] Define `__cxxabiv1::__fundamental_type_info` for builtins
  - [ ] Define `__cxxabiv1::__class_type_info` for classes
  - [ ] Define `__cxxabiv1::__si_class_type_info` for single inheritance
- [ ] Export `typeinfo for int`, `typeinfo for char`, etc. as global symbols
- [ ] C++ test: `typeid(int).name()` — verify it doesn't crash

## Final Validation

- [ ] Build staticlib: `cargo build --release`
- [ ] Build cdylib: verify `.so` exports with `nm -D target/release/liblibcplusplus.so`
- [ ] Compile and link a C++ program that uses: `new`/`delete`, `std::string`, `std::vector<int>`, `std::cout`, `std::shared_ptr`
- [ ] Run under Valgrind — verify no leaks in the happy path
- [ ] Compare exported symbol list against real libc++.so for covered features
- [ ] Document any known ABI mismatches or missing symbols

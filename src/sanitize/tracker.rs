use crate::sanitize::spinlock::SpinLock;

const CAPACITY: usize = 16384;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AllocKind {
    Rust = 0,
    ScalarNew = 1,
    ArrayNew = 2,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum SlotState {
    Empty = 0,
    Occupied = 1,
    Tombstone = 2,
}

#[derive(Clone, Copy)]
struct Entry {
    addr: usize,
    size: usize,
    state: SlotState,
    kind: AllocKind,
}

impl Entry {
    const EMPTY: Self = Self {
        addr: 0,
        size: 0,
        state: SlotState::Empty,
        kind: AllocKind::Rust,
    };
}

struct TrackerInner {
    entries: [Entry; CAPACITY],
    count: usize,
}

impl TrackerInner {
    const fn new() -> Self {
        Self {
            entries: [Entry::EMPTY; CAPACITY],
            count: 0,
        }
    }

    /// Fibonacci hashing — good distribution for pointer addresses.
    fn hash(addr: usize) -> usize {
        addr.wrapping_mul(0x9E3779B97F4A7C15) >> (usize::BITS - 14)
    }

    fn insert(&mut self, addr: usize, size: usize, kind: AllocKind) {
        let mut idx = Self::hash(addr) % CAPACITY;
        for _ in 0..CAPACITY {
            match self.entries[idx].state {
                SlotState::Empty | SlotState::Tombstone => {
                    self.entries[idx] = Entry {
                        addr,
                        size,
                        state: SlotState::Occupied,
                        kind,
                    };
                    self.count += 1;
                    return;
                }
                SlotState::Occupied => {
                    idx = (idx + 1) % CAPACITY;
                }
            }
        }
        // Table full — silently drop. Sanitizer degrades but doesn't crash.
    }

    fn remove(&mut self, addr: usize) -> Option<(usize, AllocKind)> {
        let mut idx = Self::hash(addr) % CAPACITY;
        for _ in 0..CAPACITY {
            match self.entries[idx].state {
                SlotState::Occupied if self.entries[idx].addr == addr => {
                    let size = self.entries[idx].size;
                    let kind = self.entries[idx].kind;
                    self.entries[idx].state = SlotState::Tombstone;
                    self.count -= 1;
                    return Some((size, kind));
                }
                SlotState::Empty => return None,
                _ => idx = (idx + 1) % CAPACITY,
            }
        }
        None
    }

    fn lookup(&self, addr: usize) -> Option<(usize, AllocKind)> {
        let mut idx = Self::hash(addr) % CAPACITY;
        for _ in 0..CAPACITY {
            match self.entries[idx].state {
                SlotState::Occupied if self.entries[idx].addr == addr => {
                    return Some((self.entries[idx].size, self.entries[idx].kind));
                }
                SlotState::Empty => return None,
                _ => idx = (idx + 1) % CAPACITY,
            }
        }
        None
    }

    /// Walk all live allocations, calling `f` for each. Used for leak reporting.
    fn for_each_live(&self, mut f: impl FnMut(usize, usize, AllocKind)) {
        for entry in &self.entries {
            if entry.state == SlotState::Occupied {
                f(entry.addr, entry.size, entry.kind);
            }
        }
    }
}

static TRACKER: SpinLock<TrackerInner> = SpinLock::new(TrackerInner::new());

pub fn insert(addr: usize, size: usize, kind: AllocKind) {
    TRACKER.lock().insert(addr, size, kind);
}

pub fn remove(addr: usize) -> Option<(usize, AllocKind)> {
    TRACKER.lock().remove(addr)
}

pub fn lookup(addr: usize) -> Option<(usize, AllocKind)> {
    TRACKER.lock().lookup(addr)
}

/// Report all live (unfreed) allocations. Called at program exit for leak detection.
pub fn report_leaks() {
    let guard = TRACKER.lock();
    if guard.count == 0 {
        return;
    }
    crate::sanitize::diagnostic::write_stderr(
        b"\n\x1b[1;33m=== libcplusplus sanitizer: leak report ===\x1b[0m\n",
    );
    guard.for_each_live(|addr, size, kind| {
        crate::sanitize::diagnostic::leak_detected(addr, size, kind);
    });
    let mut dec_buf = [0u8; 20];
    crate::sanitize::diagnostic::write_stderr(b"  total leaks: ");
    crate::sanitize::diagnostic::write_stderr(crate::sanitize::diagnostic::format_dec(
        guard.count,
        &mut dec_buf,
    ));
    crate::sanitize::diagnostic::write_stderr(b"\n\n");
}

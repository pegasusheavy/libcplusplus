use crate::sanitize::spinlock::SpinLock;

const CAPACITY: usize = 256;

#[derive(Clone, Copy)]
struct Entry {
    user_addr: usize,
    base_addr: usize,
    user_size: usize,
}

impl Entry {
    const EMPTY: Self = Self {
        user_addr: 0,
        base_addr: 0,
        user_size: 0,
    };
}

struct QuarantineInner {
    ring: [Entry; CAPACITY],
    pos: usize,
    len: usize,
}

impl QuarantineInner {
    const fn new() -> Self {
        Self {
            ring: [Entry::EMPTY; CAPACITY],
            pos: 0,
            len: 0,
        }
    }

    /// Push a freed block into quarantine.
    /// Returns the evicted entry's base_addr if the ring was full.
    fn push(&mut self, user_addr: usize, base_addr: usize, user_size: usize) -> Option<usize> {
        let evicted = if self.len == CAPACITY {
            Some(self.ring[self.pos].base_addr)
        } else {
            self.len += 1;
            None
        };

        self.ring[self.pos] = Entry {
            user_addr,
            base_addr,
            user_size,
        };
        self.pos = (self.pos + 1) % CAPACITY;

        evicted
    }

    /// Check if an address was recently freed (linear scan).
    fn contains(&self, user_addr: usize) -> bool {
        for i in 0..self.len {
            if self.ring[i].user_addr == user_addr {
                return true;
            }
        }
        false
    }
}

static QUARANTINE: SpinLock<QuarantineInner> = SpinLock::new(QuarantineInner::new());

/// Quarantine a freed block. Returns the evicted base address to actually free, if any.
pub fn push(user_addr: usize, base_addr: usize, user_size: usize) -> Option<usize> {
    QUARANTINE.lock().push(user_addr, base_addr, user_size)
}

/// Check if an address was recently freed (is still in quarantine).
pub fn contains(user_addr: usize) -> bool {
    QUARANTINE.lock().contains(user_addr)
}

use core::sync::atomic::{AtomicU64, Ordering};

/// Generation counter for detecting iterator invalidation.
///
/// Containers increment their epoch on every mutating operation.
/// Iterators capture the current epoch on creation and compare
/// on dereference — a mismatch means the iterator is invalidated.
pub struct Epoch(AtomicU64);

impl Default for Epoch {
    fn default() -> Self {
        Self::new()
    }
}

impl Epoch {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    /// Read the current generation.
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }

    /// Increment the generation. Returns the previous value.
    pub fn bump(&self) -> u64 {
        self.0.fetch_add(1, Ordering::AcqRel)
    }
}

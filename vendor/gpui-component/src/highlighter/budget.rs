use std::{
    cell::Cell,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// Cooperative execution/projection budget, shared across both diff sides.
/// Fixed bundled query construction is cached but not interruptible; it is
/// measured separately, so cold initialization cannot erase valid captures.
pub(crate) struct SyntaxBudget<'a> {
    start: Instant,
    limit: Duration,
    initialization: Cell<Duration>,
    cancelled: &'a AtomicBool,
}

impl<'a> SyntaxBudget<'a> {
    pub(crate) fn new(limit: Duration, cancelled: &'a AtomicBool) -> Self {
        Self {
            start: Instant::now(),
            limit,
            initialization: Cell::new(Duration::ZERO),
            cancelled,
        }
    }

    pub(crate) fn exhausted(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
            || self
                .start
                .elapsed()
                .saturating_sub(self.initialization.get())
                >= self.limit
    }

    pub(crate) fn initialize<T>(&self, build: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let value = build();
        self.initialization
            .set(self.initialization.get() + start.elapsed());
        value
    }
}

//! Cooperative cancellation shared by callers, the gateway, and provider streams.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A cloneable cancellation flag.
///
/// The token never blocks. Providers must check [`CancellationToken::is_cancelled`] before starting
/// I/O and before each event poll. The gateway checks it before every provider call and event read,
/// so a compliant provider observes cancellation within the configured event-poll budget.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    is_cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a token in the running state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Calling this method more than once is idempotent.
    pub fn cancel(&self) {
        self.is_cancelled.store(true, Ordering::Release);
    }

    /// Returns whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled.load(Ordering::Acquire)
    }
}

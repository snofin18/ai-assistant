//! Pure heartbeat watchdog state.
//!
//! Time is supplied by the caller so tests can advance a virtual clock and the
//! host can use the same state machine without hiding a wall-clock dependency.

use std::time::Duration;

use crate::{IpcError, IpcResult};

/// Tracks the last observed activity and rejects a silent peer.
#[derive(Debug, Clone)]
pub struct HeartbeatMonitor {
    timeout: Duration,
    last_activity_unix_ms: u64,
}

impl HeartbeatMonitor {
    /// Create a monitor with activity already observed at `start_unix_ms`.
    #[must_use]
    pub const fn new(timeout: Duration, start_unix_ms: u64) -> Self {
        Self {
            timeout,
            last_activity_unix_ms: start_unix_ms,
        }
    }

    /// Record a heartbeat, response, or request as activity.
    pub const fn observe(&mut self, observed_unix_ms: u64) {
        self.last_activity_unix_ms = observed_unix_ms;
    }

    /// Return the remaining time before the watchdog fires.
    #[must_use]
    pub fn remaining(&self, now_unix_ms: u64) -> Duration {
        let elapsed_ms = now_unix_ms.saturating_sub(self.last_activity_unix_ms);
        let timeout_ms = u64::try_from(self.timeout.as_millis()).unwrap_or(u64::MAX);
        Duration::from_millis(timeout_ms.saturating_sub(elapsed_ms))
    }

    /// Check whether the peer is still within the liveness window.
    ///
    /// # Errors
    /// Returns [`IpcError::Timeout`] with operation `heartbeat` after silence.
    pub fn check(&self, now_unix_ms: u64) -> IpcResult<()> {
        let elapsed_ms = now_unix_ms.saturating_sub(self.last_activity_unix_ms);
        let timeout_ms = u64::try_from(self.timeout.as_millis()).unwrap_or(u64::MAX);
        if elapsed_ms >= timeout_ms {
            Err(IpcError::timeout("heartbeat", self.timeout))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::HeartbeatMonitor;
    use crate::IpcError;

    #[test]
    fn test_heartbeat_monitor_accepts_activity_before_deadline() {
        let mut monitor = HeartbeatMonitor::new(Duration::from_millis(100), 1_000);
        monitor.observe(1_050);
        assert!(monitor.check(1_099).is_ok());
    }

    #[test]
    fn test_heartbeat_monitor_rejects_silence_at_deadline() {
        let monitor = HeartbeatMonitor::new(Duration::from_millis(100), 1_000);
        assert!(matches!(
            monitor.check(1_100),
            Err(IpcError::Timeout {
                operation: "heartbeat",
                ..
            })
        ));
    }

    #[test]
    fn test_heartbeat_remaining_saturates_at_zero() {
        let monitor = HeartbeatMonitor::new(Duration::from_millis(100), 1_000);
        assert_eq!(monitor.remaining(1_500), Duration::ZERO);
    }
}

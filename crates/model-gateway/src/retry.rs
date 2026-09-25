//! Injected clock, sleep, and exponential-jitter retry policy.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{ModelGatewayError, ModelResult};
use crate::identity::DurationMs;

/// Retry limits and exponential-backoff bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_retries: u32,
    base_delay: DurationMs,
    max_delay: DurationMs,
}

impl RetryPolicy {
    /// Creates a retry policy.
    #[must_use]
    pub const fn new(max_retries: u32, base_delay: DurationMs, max_delay: DurationMs) -> Self {
        Self {
            max_retries,
            base_delay,
            max_delay,
        }
    }

    /// Returns the maximum retry count after the first attempt.
    #[must_use]
    pub const fn max_retries(self) -> u32 {
        self.max_retries
    }

    /// Validates retry bounds.
    ///
    /// # Errors
    ///
    /// Returns `InvalidConfiguration` when retry count or delays are outside supported bounds.
    pub fn validate(self) -> ModelResult<()> {
        if self.max_retries > 16 {
            return Err(ModelGatewayError::InvalidConfiguration {
                reason: "max_retries must not exceed 16".to_string(),
            });
        }
        if self.base_delay.is_zero() {
            return Err(ModelGatewayError::InvalidConfiguration {
                reason: "base_delay must be positive".to_string(),
            });
        }
        if self.max_delay < self.base_delay {
            return Err(ModelGatewayError::InvalidConfiguration {
                reason: "max_delay must be greater than or equal to base_delay".to_string(),
            });
        }
        Ok(())
    }

    /// Computes the delay before one retry.
    ///
    /// `retry_index` is zero for the delay before the first retry. `jitter_permille` must be in
    /// `-1000..=1000`; the resulting delay is clamped to `max_delay`.
    ///
    /// # Errors
    ///
    /// Returns `InvalidConfiguration` for an out-of-range jitter value or `NumericOverflow` when
    /// multiplication cannot be represented.
    pub fn delay_for_retry(
        self,
        retry_index: u32,
        jitter_permille: i32,
    ) -> ModelResult<DurationMs> {
        if !(-1000..=1000).contains(&jitter_permille) {
            return Err(ModelGatewayError::InvalidConfiguration {
                reason: "jitter_permille must be within -1000..=1000".to_string(),
            });
        }
        let exponent = retry_index.min(63);
        let multiplier = 1_u64
            .checked_shl(exponent)
            .ok_or(ModelGatewayError::NumericOverflow {
                field: "retry_delay",
            })?;
        let base = self.base_delay.get().checked_mul(multiplier).ok_or(
            ModelGatewayError::NumericOverflow {
                field: "retry_delay",
            },
        )?;
        let adjusted = if jitter_permille >= 0 {
            let numerator = base
                .checked_mul(u64::try_from(1000 + jitter_permille).map_err(|_| {
                    ModelGatewayError::NumericOverflow {
                        field: "retry_jitter",
                    }
                })?)
                .ok_or(ModelGatewayError::NumericOverflow {
                    field: "retry_jitter",
                })?;
            numerator / 1000
        } else {
            let reduction = u64::try_from(-jitter_permille).map_err(|_| {
                ModelGatewayError::NumericOverflow {
                    field: "retry_jitter",
                }
            })?;
            let numerator = base.checked_mul(1000_u64.saturating_sub(reduction)).ok_or(
                ModelGatewayError::NumericOverflow {
                    field: "retry_jitter",
                },
            )?;
            numerator / 1000
        };
        Ok(DurationMs::new(adjusted.min(self.max_delay.get())))
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_delay: DurationMs::new(250),
            max_delay: DurationMs::new(2_000),
        }
    }
}

/// Injectable deterministic jitter source.
pub trait JitterSource: Send + Sync {
    /// Returns a jitter adjustment in permille for the zero-based retry index.
    fn jitter_permille(&self, retry_index: u32) -> i32;
}

/// Jitter source that leaves exponential backoff unchanged.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoJitter;

impl JitterSource for NoJitter {
    fn jitter_permille(&self, _retry_index: u32) -> i32 {
        0
    }
}

/// Injectable monotonic clock used for latency measurements.
pub trait MonotonicClock: Send + Sync {
    /// Returns monotonic milliseconds since an unspecified process-local epoch.
    fn now_ms(&self) -> u64;
}

/// Process-local monotonic clock.
#[derive(Debug, Clone)]
pub struct SystemMonotonicClock {
    started_at: Instant,
}

impl Default for SystemMonotonicClock {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }
}

impl MonotonicClock for SystemMonotonicClock {
    fn now_ms(&self) -> u64 {
        // The process-local monotonic epoch cannot exceed u64 milliseconds during any realistic
        // runtime; saturating here avoids wrapping if the process exceeds that astronomical age.
        u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Injectable sleeper used by retry backoff.
pub trait Sleeper: Send + Sync {
    /// Blocks for the requested duration.
    fn sleep(&self, duration: DurationMs);
}

/// Thread sleeper for production use.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThreadSleeper;

impl Sleeper for ThreadSleeper {
    fn sleep(&self, duration: DurationMs) {
        std::thread::sleep(Duration::from_millis(duration.get()));
    }
}

/// Shared runtime dependencies for the gateway.
#[derive(Clone)]
pub struct GatewayRuntime {
    pub(crate) clock: Arc<dyn MonotonicClock>,
    pub(crate) sleeper: Arc<dyn Sleeper>,
    pub(crate) jitter: Arc<dyn JitterSource>,
}

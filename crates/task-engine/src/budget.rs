//! Task budgets and usage accounting.

use serde::{Deserialize, Serialize};

use crate::error::{TaskEngineError, TaskEngineResult};

/// Maximum resources allocated to one task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Budget {
    /// Maximum number of steps that may start.
    pub max_steps: u64,
    /// Maximum wall-clock time from task creation.
    pub max_elapsed_ms: u64,
    /// Maximum combined input and output tokens.
    pub max_tokens: u64,
    /// Maximum accumulated cost in USD.
    pub max_cost_usd: f64,
}

impl Budget {
    /// Creates a validated budget.
    ///
    /// # Errors
    ///
    /// Returns [`TaskEngineError::InvalidPlan`] when `max_steps` or
    /// `max_elapsed_ms` is zero, or when `max_cost_usd` is negative,
    /// infinite, or NaN.
    pub fn new(
        max_steps: u64,
        max_elapsed_ms: u64,
        max_tokens: u64,
        max_cost_usd: f64,
    ) -> TaskEngineResult<Self> {
        if max_steps == 0 {
            return Err(TaskEngineError::InvalidPlan {
                reason: "budget.max_steps must be greater than zero".to_owned(),
            });
        }
        if max_elapsed_ms == 0 {
            return Err(TaskEngineError::InvalidPlan {
                reason: "budget.max_elapsed_ms must be greater than zero".to_owned(),
            });
        }
        if !max_cost_usd.is_finite() || max_cost_usd < 0.0 {
            return Err(TaskEngineError::InvalidPlan {
                reason: "budget.max_cost_usd must be finite and non-negative".to_owned(),
            });
        }
        Ok(Self {
            max_steps,
            max_elapsed_ms,
            max_tokens,
            max_cost_usd,
        })
    }

    /// Checks usage plus externally measured elapsed time.
    #[must_use]
    pub fn check(&self, usage: &BudgetUsage, elapsed_ms: u64) -> BudgetCheck {
        if usage.steps_started > self.max_steps {
            return BudgetCheck::Exceeded(BudgetLimit::Steps);
        }
        if elapsed_ms > self.max_elapsed_ms {
            return BudgetCheck::Exceeded(BudgetLimit::ElapsedTime);
        }
        match usage.total_tokens() {
            Ok(total) if total > self.max_tokens => {
                return BudgetCheck::Exceeded(BudgetLimit::Tokens);
            }
            Err(_) => return BudgetCheck::Exceeded(BudgetLimit::Tokens),
            Ok(_) => {}
        }
        if usage.cost_usd > self.max_cost_usd {
            return BudgetCheck::Exceeded(BudgetLimit::Cost);
        }
        BudgetCheck::Within
    }
}

/// Accumulated task usage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct BudgetUsage {
    /// Number of steps that entered prechecking.
    pub steps_started: u64,
    /// Elapsed time at the last persisted checkpoint.
    pub elapsed_ms: u64,
    /// Accumulated input tokens.
    pub tokens_in: u64,
    /// Accumulated output tokens.
    pub tokens_out: u64,
    /// Accumulated cost in USD.
    pub cost_usd: f64,
}

impl BudgetUsage {
    /// Returns `tokens_in + tokens_out` without silent saturation.
    ///
    /// # Errors
    ///
    /// Returns [`TaskEngineError::NumericOverflow`] if the sum exceeds
    /// `u64::MAX`.
    pub fn total_tokens(&self) -> TaskEngineResult<u64> {
        self.tokens_in
            .checked_add(self.tokens_out)
            .ok_or(TaskEngineError::NumericOverflow {
                field: "budget_usage.tokens",
            })
    }

    /// Records one started step.
    ///
    /// # Errors
    ///
    /// Returns [`TaskEngineError::NumericOverflow`] on counter overflow.
    pub fn record_step_started(&mut self) -> TaskEngineResult<()> {
        self.steps_started =
            self.steps_started
                .checked_add(1)
                .ok_or(TaskEngineError::NumericOverflow {
                    field: "budget_usage.steps_started",
                })?;
        Ok(())
    }

    /// Records token and cost usage.
    ///
    /// # Errors
    ///
    /// Returns [`TaskEngineError::NumericOverflow`] on token counter overflow
    /// or [`TaskEngineError::InvalidPlan`] for a negative or non-finite cost.
    pub fn record_usage(
        &mut self,
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
    ) -> TaskEngineResult<()> {
        if !cost_usd.is_finite() || cost_usd < 0.0 {
            return Err(TaskEngineError::InvalidPlan {
                reason: "usage cost must be finite and non-negative".to_owned(),
            });
        }
        self.tokens_in =
            self.tokens_in
                .checked_add(tokens_in)
                .ok_or(TaskEngineError::NumericOverflow {
                    field: "budget_usage.tokens_in",
                })?;
        self.tokens_out =
            self.tokens_out
                .checked_add(tokens_out)
                .ok_or(TaskEngineError::NumericOverflow {
                    field: "budget_usage.tokens_out",
                })?;
        self.cost_usd += cost_usd;
        if !self.cost_usd.is_finite() {
            return Err(TaskEngineError::NumericOverflow {
                field: "budget_usage.cost_usd",
            });
        }
        Ok(())
    }
}

/// Result of a budget check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BudgetCheck {
    /// Usage remains within every bound.
    Within,
    /// One deterministic bound was exceeded.
    Exceeded(BudgetLimit),
}

/// The first exhausted budget dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BudgetLimit {
    /// Maximum started steps.
    Steps,
    /// Maximum elapsed wall-clock time.
    ElapsedTime,
    /// Maximum combined tokens.
    Tokens,
    /// Maximum cost.
    Cost,
}

/// One externally measured usage increment.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UsageDelta {
    /// Input tokens consumed by the step.
    pub tokens_in: u64,
    /// Output tokens produced by the step.
    pub tokens_out: u64,
    /// Monetary cost in USD.
    pub cost_usd: f64,
}

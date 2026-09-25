//! Integer model pricing and cost computation.

use crate::error::{ModelGatewayError, ModelResult};
use crate::identity::{CostMicroUsd, TokenCount};
use crate::model::Usage;

/// Integer provider pricing per one million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pricing {
    /// Micro-USD charged per one million fresh input tokens.
    pub input_micro_usd_per_million: u64,
    /// Micro-USD charged per one million cached input tokens.
    pub cached_input_micro_usd_per_million: u64,
    /// Micro-USD charged per one million output tokens.
    pub output_micro_usd_per_million: u64,
}

impl Pricing {
    /// Creates pricing values.
    #[must_use]
    pub const fn new(
        input_micro_usd_per_million: u64,
        cached_input_micro_usd_per_million: u64,
        output_micro_usd_per_million: u64,
    ) -> Self {
        Self {
            input_micro_usd_per_million,
            cached_input_micro_usd_per_million,
            output_micro_usd_per_million,
        }
    }

    /// Computes integer micro-USD cost.
    ///
    /// Each component rounds upward, so a non-zero usage never becomes a zero-cost record. The
    /// cached count is subtracted from total input tokens before applying the fresh-input rate.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` for inconsistent usage or `NumericOverflow` when a component or
    /// total cannot be represented as `u64` micro-USD.
    pub fn compute_cost(&self, usage: &Usage) -> ModelResult<CostMicroUsd> {
        usage.validate()?;
        let fresh_input_tokens =
            TokenCount::new(usage.input_tokens.get() - usage.cached_input_tokens.get());
        let fresh_cost = component_cost(fresh_input_tokens, self.input_micro_usd_per_million)?;
        let cached_cost = component_cost(
            usage.cached_input_tokens,
            self.cached_input_micro_usd_per_million,
        )?;
        let output_cost = component_cost(usage.output_tokens, self.output_micro_usd_per_million)?;
        fresh_cost
            .checked_add(cached_cost)?
            .checked_add(output_cost)
    }
}

fn component_cost(
    tokens: TokenCount,
    rate_micro_usd_per_million: u64,
) -> ModelResult<CostMicroUsd> {
    let numerator = u128::from(tokens.get())
        .checked_mul(u128::from(rate_micro_usd_per_million))
        .ok_or(ModelGatewayError::NumericOverflow { field: "pricing" })?;
    let rounded = numerator
        .checked_add(999_999)
        .ok_or(ModelGatewayError::NumericOverflow { field: "pricing" })?
        / 1_000_000;
    let converted = u64::try_from(rounded)
        .map_err(|_| ModelGatewayError::NumericOverflow { field: "pricing" })?;
    Ok(CostMicroUsd::new(converted))
}

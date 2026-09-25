//! Per-model cost aggregation for task and UI cost panels.

use std::collections::BTreeMap;

use crate::error::ModelResult;
use crate::identity::{CostMicroUsd, ModelId, TokenCount};
use crate::model::UsageRecord;

/// Aggregated cost and token totals for one model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CostTotal {
    /// Number of recorded calls.
    pub call_count: u64,
    /// Input tokens including cached input.
    pub input_tokens: TokenCount,
    /// Output tokens.
    pub output_tokens: TokenCount,
    /// Cost in integer micro-USD.
    pub cost: CostMicroUsd,
}

/// Append-only cost aggregator for the current task, day, or month.
#[derive(Debug, Clone, Default)]
pub struct CostLedger {
    records: Vec<UsageRecord>,
    totals_by_model: BTreeMap<ModelId, CostTotal>,
}

impl CostLedger {
    /// Records one usage record.
    ///
    /// Returns `NumericOverflow` before changing the ledger when any aggregate would overflow.
    ///
    /// # Errors
    ///
    /// Returns `NumericOverflow` when a per-model aggregate cannot be updated.
    pub fn record(&mut self, usage_record: UsageRecord) -> ModelResult<()> {
        let model_id = usage_record.model_id.clone();
        let current = self
            .totals_by_model
            .get(&model_id)
            .copied()
            .unwrap_or_default();
        let next = CostTotal {
            call_count: current.call_count.checked_add(1).ok_or(
                crate::ModelGatewayError::NumericOverflow {
                    field: "cost_ledger.call_count",
                },
            )?,
            input_tokens: current
                .input_tokens
                .checked_add(usage_record.usage.input_tokens)?,
            output_tokens: current
                .output_tokens
                .checked_add(usage_record.usage.output_tokens)?,
            cost: current.cost.checked_add(usage_record.cost)?,
        };
        self.totals_by_model.insert(model_id, next);
        self.records.push(usage_record);
        Ok(())
    }

    /// Returns the recorded calls.
    #[must_use]
    pub fn records(&self) -> &[UsageRecord] {
        &self.records
    }

    /// Returns one model's aggregate, or `None` when no call has been recorded.
    #[must_use]
    pub fn total_for_model(&self, model_id: &ModelId) -> Option<CostTotal> {
        self.totals_by_model.get(model_id).copied()
    }

    /// Returns aggregate totals across all models.
    ///
    /// # Errors
    ///
    /// Returns `NumericOverflow` when any cross-model aggregate cannot be updated.
    pub fn total(&self) -> ModelResult<CostTotal> {
        let mut total = CostTotal::default();
        for model_total in self.totals_by_model.values() {
            total.call_count = total.call_count.checked_add(model_total.call_count).ok_or(
                crate::ModelGatewayError::NumericOverflow {
                    field: "cost_ledger.call_count",
                },
            )?;
            total.input_tokens = total.input_tokens.checked_add(model_total.input_tokens)?;
            total.output_tokens = total.output_tokens.checked_add(model_total.output_tokens)?;
            total.cost = total.cost.checked_add(model_total.cost)?;
        }
        Ok(total)
    }
}

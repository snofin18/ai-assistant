//! Binary-layer task dataflow for ADR-0061.
//!
//! Responsibilities:
//! - describe the condition and outputs of each executable task-package step;
//! - resolve whole-value `$name` references against an execution context;
//! - parse the closed `when` predicate subset used by the 1a task packages.
//!
//! Boundaries:
//! - does not execute tools or touch the platform;
//! - does not introduce a general expression language: partial interpolation,
//!   arithmetic, functions, parentheses and logical OR are rejected;
//! - does not persist context; one run owns one in-memory state.
//!
//! Invariants:
//! 1. `$name` is accepted only when the entire string is that reference;
//! 2. unknown references and type-incompatible comparisons fail closed;
//! 3. every parsed condition consists only of the documented closed predicates.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Number, Value};
use thiserror::Error;

/// One executable step's dataflow metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeStepBinding {
    /// Tool selected for the step.
    pub tool: String,
    /// Declared `when` predicate, already validated.
    pub condition: Option<ConditionExpr>,
    /// Output names published after the step commits.
    pub outputs: Vec<String>,
}

/// Dataflow metadata for one task package, kept out of Plan JSON.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeDataflowPlan {
    steps: BTreeMap<String, RuntimeStepBinding>,
}

impl RuntimeDataflowPlan {
    /// Creates an empty plan.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one executable step binding.
    ///
    /// # Errors
    ///
    /// Returns [`DataflowError::DuplicateStep`] when the same step id is added
    /// twice.
    pub fn insert(
        &mut self,
        step_id: impl Into<String>,
        binding: RuntimeStepBinding,
    ) -> Result<(), DataflowError> {
        let step_id = step_id.into();
        if self.steps.contains_key(&step_id) {
            return Err(DataflowError::DuplicateStep { step_id });
        }
        self.steps.insert(step_id, binding);
        Ok(())
    }

    /// Returns one binding by step id.
    #[must_use]
    pub fn get(&self, step_id: &str) -> Option<&RuntimeStepBinding> {
        self.steps.get(step_id)
    }

    /// Returns all step ids in deterministic order.
    #[must_use]
    pub fn step_ids(&self) -> Vec<String> {
        self.steps.keys().cloned().collect()
    }
}

/// Why a dataflow declaration or value resolution is invalid.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum DataflowError {
    /// The same step id was declared twice.
    #[error("dataflow step `{step_id}` is declared more than once")]
    DuplicateStep {
        /// Duplicate step id.
        step_id: String,
    },

    /// A condition uses syntax outside the closed predicate subset.
    #[error("condition `{condition}` is outside the closed predicate subset: {reason}")]
    InvalidCondition {
        /// Original condition text.
        condition: String,
        /// Why it was rejected.
        reason: String,
    },

    /// A reference has no value in the execution context.
    #[error("reference `{reference}` is not defined in the task execution context")]
    UnknownReference {
        /// Missing reference, including the leading `$`.
        reference: String,
    },

    /// A reference is embedded in a larger string.
    #[error(
        "reference `{reference}` must occupy the whole string; partial interpolation is not supported"
    )]
    PartialInterpolation {
        /// Offending value.
        reference: String,
    },

    /// A condition comparison cannot be evaluated with the supplied types.
    #[error("condition `{condition}` cannot compare {left_type} with {right_type}")]
    TypeMismatch {
        /// Original condition text.
        condition: String,
        /// Left operand type.
        left_type: String,
        /// Right operand type.
        right_type: String,
    },

    /// A condition expected a boolean value.
    #[error("condition `{condition}` requires a boolean value for `{reference}`")]
    NonBooleanCondition {
        /// Original condition text.
        condition: String,
        /// Reference that was not boolean.
        reference: String,
    },
}

/// Closed condition expression used by task-package `when` declarations.
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionExpr {
    /// A boolean reference, optionally negated.
    Boolean {
        /// Referenced name without `$`.
        name: String,
        /// Whether the boolean is negated.
        negated: bool,
    },
    /// A comparison between two operands.
    Compare {
        /// Left operand.
        left: ConditionOperand,
        /// Comparison operator.
        operator: ConditionOperator,
        /// Right operand.
        right: ConditionOperand,
    },
    /// Conjunction of at least two predicates.
    All(Vec<Self>),
}

impl ConditionExpr {
    /// Evaluates the condition against an execution context.
    ///
    /// # Errors
    ///
    /// Returns [`DataflowError`] for missing references or incompatible types.
    pub fn evaluate(
        &self,
        context: &Map<String, Value>,
        source: &str,
    ) -> Result<bool, DataflowError> {
        match self {
            Self::Boolean { name, negated } => {
                let value = context
                    .get(name)
                    .ok_or_else(|| DataflowError::UnknownReference {
                        reference: format!("${name}"),
                    })?;
                let value = value
                    .as_bool()
                    .ok_or_else(|| DataflowError::NonBooleanCondition {
                        condition: source.to_owned(),
                        reference: name.clone(),
                    })?;
                Ok(if *negated { !value } else { value })
            }
            Self::Compare {
                left,
                operator,
                right,
            } => evaluate_comparison(context, source, left, *operator, right),
            Self::All(parts) => {
                for part in parts {
                    if !part.evaluate(context, source)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }

    /// Returns every reference used by this condition.
    #[must_use]
    pub fn references(&self) -> BTreeSet<String> {
        let mut references = BTreeSet::new();
        self.collect_references(&mut references);
        references
    }

    fn collect_references(&self, references: &mut BTreeSet<String>) {
        match self {
            Self::Boolean { name, .. } => {
                references.insert(name.clone());
            }
            Self::Compare { left, right, .. } => {
                left.collect_references(references);
                right.collect_references(references);
            }
            Self::All(parts) => {
                for part in parts {
                    part.collect_references(references);
                }
            }
        }
    }
}

/// Operand allowed inside a condition comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionOperand {
    /// Reference by name without `$`.
    Reference(String),
    /// String literal.
    String(String),
    /// Number literal.
    Number(Number),
    /// Boolean literal.
    Boolean(bool),
}

impl ConditionOperand {
    fn collect_references(&self, references: &mut BTreeSet<String>) {
        if let Self::Reference(name) = self {
            references.insert(name.clone());
        }
    }

    fn value(&self, context: &Map<String, Value>) -> Result<Value, DataflowError> {
        match self {
            Self::Reference(name) => {
                context
                    .get(name)
                    .cloned()
                    .ok_or_else(|| DataflowError::UnknownReference {
                        reference: format!("${name}"),
                    })
            }
            Self::String(value) => Ok(Value::String(value.clone())),
            Self::Number(value) => Ok(Value::Number(value.clone())),
            Self::Boolean(value) => Ok(Value::Bool(*value)),
        }
    }
}

/// Comparison operators allowed by the closed predicate subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionOperator {
    /// Equality.
    Equal,
    /// Inequality.
    NotEqual,
    /// Numeric/string less-than.
    Less,
    /// Numeric/string less-than-or-equal.
    LessOrEqual,
    /// Numeric/string greater-than.
    Greater,
    /// Numeric/string greater-than-or-equal.
    GreaterOrEqual,
}

/// Parses one task-package `when` string.
///
/// # Errors
///
/// Returns [`DataflowError::InvalidCondition`] when the expression uses syntax
/// outside the closed predicate subset.
pub fn parse_condition(source: &str) -> Result<ConditionExpr, DataflowError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(invalid_condition(source, "condition is empty"));
    }
    if source.contains("||") || source.contains('(') || source.contains(')') {
        return Err(invalid_condition(
            source,
            "logical OR, parentheses and calls are not supported",
        ));
    }
    let mut parts = Vec::new();
    for raw in source.split("&&") {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(invalid_condition(source, "empty conjunction term"));
        }
        parts.push(parse_condition_term(raw, source)?);
    }
    if parts.len() == 1 {
        Ok(parts.remove(0))
    } else {
        Ok(ConditionExpr::All(parts))
    }
}

fn parse_condition_term(term: &str, source: &str) -> Result<ConditionExpr, DataflowError> {
    if let Some(name) = term.strip_prefix('!') {
        let name = validate_reference_name(name, source)?;
        return Ok(ConditionExpr::Boolean {
            name,
            negated: true,
        });
    }
    for (operator_text, operator) in [
        ("==", ConditionOperator::Equal),
        ("!=", ConditionOperator::NotEqual),
        ("<=", ConditionOperator::LessOrEqual),
        (">=", ConditionOperator::GreaterOrEqual),
        ("<", ConditionOperator::Less),
        (">", ConditionOperator::Greater),
    ] {
        if let Some(index) = term.find(operator_text) {
            let left = term[..index].trim();
            let right = term[index + operator_text.len()..].trim();
            if left.is_empty() || right.is_empty() {
                return Err(invalid_condition(source, "comparison operand is empty"));
            }
            return Ok(ConditionExpr::Compare {
                left: parse_operand(left),
                operator,
                right: parse_operand(right),
            });
        }
    }
    let name = validate_reference_name(term, source)?;
    Ok(ConditionExpr::Boolean {
        name,
        negated: false,
    })
}

fn parse_operand(token: &str) -> ConditionOperand {
    if token == "true" {
        return ConditionOperand::Boolean(true);
    }
    if token == "false" {
        return ConditionOperand::Boolean(false);
    }
    if let Ok(number) = token.parse::<i64>() {
        return ConditionOperand::Number(Number::from(number));
    }
    if let Ok(number) = token.parse::<u64>() {
        return ConditionOperand::Number(Number::from(number));
    }
    if let Ok(number) = token.parse::<f64>()
        && let Some(number) = Number::from_f64(number)
    {
        return ConditionOperand::Number(number);
    }
    if let Ok(value) = serde_json::from_str::<String>(token) {
        return ConditionOperand::String(value);
    }
    ConditionOperand::Reference(token.to_owned())
}

fn validate_reference_name(name: &str, source: &str) -> Result<String, DataflowError> {
    let name = name.trim();
    if is_reference_name(name) {
        Ok(name.to_owned())
    } else {
        Err(invalid_condition(
            source,
            "boolean predicate must be a bare reference name",
        ))
    }
}

fn evaluate_comparison(
    context: &Map<String, Value>,
    source: &str,
    left: &ConditionOperand,
    operator: ConditionOperator,
    right: &ConditionOperand,
) -> Result<bool, DataflowError> {
    let left_value = left.value(context)?;
    let right_value = right.value(context)?;
    let ordering = compare_values(&left_value, &right_value);
    let Some(ordering) = ordering else {
        return Err(DataflowError::TypeMismatch {
            condition: source.to_owned(),
            left_type: value_type(&left_value).to_owned(),
            right_type: value_type(&right_value).to_owned(),
        });
    };
    Ok(match operator {
        ConditionOperator::Equal => ordering == std::cmp::Ordering::Equal,
        ConditionOperator::NotEqual => ordering != std::cmp::Ordering::Equal,
        ConditionOperator::Less => ordering == std::cmp::Ordering::Less,
        ConditionOperator::LessOrEqual => ordering != std::cmp::Ordering::Greater,
        ConditionOperator::Greater => ordering == std::cmp::Ordering::Greater,
        ConditionOperator::GreaterOrEqual => ordering != std::cmp::Ordering::Less,
    })
}

fn compare_values(left: &Value, right: &Value) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => left
            .as_f64()
            .zip(right.as_f64())
            .and_then(|(left, right)| left.partial_cmp(&right)),
        (Value::String(left), Value::String(right)) => Some(left.cmp(right)),
        (Value::Bool(left), Value::Bool(right)) => Some(left.cmp(right)),
        _ => None,
    }
}

const fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn invalid_condition(source: &str, reason: &str) -> DataflowError {
    DataflowError::InvalidCondition {
        condition: source.to_owned(),
        reason: reason.to_owned(),
    }
}

/// Resolves all whole-value `$name` references inside a JSON value.
///
/// # Errors
///
/// Returns [`DataflowError::UnknownReference`] for a missing name and
/// [`DataflowError::PartialInterpolation`] when a reference is embedded in a
/// larger string.
pub fn resolve_references(
    value: &Value,
    context: &Map<String, Value>,
) -> Result<Value, DataflowError> {
    match value {
        Value::String(text) if text.contains('$') => {
            if !text.starts_with('$') {
                return Err(DataflowError::PartialInterpolation {
                    reference: text.clone(),
                });
            }
            let name = text.trim_start_matches('$');
            if !is_reference_name(name) {
                return Err(DataflowError::PartialInterpolation {
                    reference: text.clone(),
                });
            }
            context
                .get(name)
                .cloned()
                .ok_or_else(|| DataflowError::UnknownReference {
                    reference: text.clone(),
                })
        }
        Value::Array(items) => items
            .iter()
            .map(|item| resolve_references(item, context))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| {
                resolve_references(value, context).map(|resolved| (key.clone(), resolved))
            })
            .collect::<Result<Map<_, _>, _>>()
            .map(Value::Object),
        _ => Ok(value.clone()),
    }
}

/// Returns every whole-value `$name` reference inside a JSON value.
///
/// # Errors
///
/// Returns [`DataflowError::PartialInterpolation`] for a malformed
/// `$`-prefixed string.
pub fn collect_references(value: &Value) -> Result<BTreeSet<String>, DataflowError> {
    let mut references = BTreeSet::new();
    collect_references_into(value, &mut references)?;
    Ok(references)
}

fn collect_references_into(
    value: &Value,
    references: &mut BTreeSet<String>,
) -> Result<(), DataflowError> {
    match value {
        Value::String(text) if text.contains('$') => {
            if !text.starts_with('$') {
                return Err(DataflowError::PartialInterpolation {
                    reference: text.clone(),
                });
            }
            let name = text.trim_start_matches('$');
            if !is_reference_name(name) {
                return Err(DataflowError::PartialInterpolation {
                    reference: text.clone(),
                });
            }
            references.insert(name.to_owned());
        }
        Value::Array(items) => {
            for item in items {
                collect_references_into(item, references)?;
            }
        }
        Value::Object(fields) => {
            for value in fields.values() {
                collect_references_into(value, references)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_reference_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '-')
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::{Map, json};

    use super::{
        ConditionExpr, DataflowError, collect_references, parse_condition, resolve_references,
    };

    #[test]
    fn test_whole_value_reference_preserves_type() {
        let context = Map::from_iter([("count".to_owned(), json!(3))]);
        let resolved = resolve_references(&json!({"value": "$count"}), &context).expect("resolve");
        assert_eq!(resolved, json!({"value": 3}));
    }

    #[test]
    fn test_partial_interpolation_is_rejected() {
        let context = Map::from_iter([("count".to_owned(), json!(3))]);
        let error = resolve_references(&json!("count=$count"), &context)
            .expect_err("partial interpolation must fail");
        assert!(matches!(error, DataflowError::PartialInterpolation { .. }));
    }

    #[test]
    fn test_closed_condition_subset_covers_the_package_forms() {
        let first = parse_condition("file_size_bytes <= max_text_bytes").expect("comparison");
        assert!(matches!(first, ConditionExpr::Compare { .. }));

        let second =
            parse_condition("!target_existed_before && file_created").expect("conjunction");
        assert!(matches!(second, ConditionExpr::All(_)));
    }

    #[test]
    fn test_condition_rejects_functions_and_logical_or() {
        assert!(parse_condition("ready() && other").is_err());
        assert!(parse_condition("ready || other").is_err());
    }

    #[test]
    fn test_condition_evaluates_negation_and_comparison() {
        let context = Map::from_iter([
            ("target_existed_before".to_owned(), json!(false)),
            ("file_created".to_owned(), json!(true)),
            ("file_size_bytes".to_owned(), json!(10)),
            ("max_text_bytes".to_owned(), json!(20)),
        ]);
        assert!(
            parse_condition("!target_existed_before && file_created")
                .expect("parse")
                .evaluate(&context, "test")
                .expect("evaluate")
        );
        assert!(
            parse_condition("file_size_bytes <= max_text_bytes")
                .expect("parse")
                .evaluate(&context, "test")
                .expect("evaluate")
        );
    }

    #[test]
    fn test_reference_collection_is_recursive_and_deterministic() {
        let references = collect_references(&json!({"a": ["$one", {"b": "$two"}], "c": "literal"}))
            .expect("collect");
        assert_eq!(
            references.into_iter().collect::<Vec<_>>(),
            vec!["one", "two"]
        );
    }
}

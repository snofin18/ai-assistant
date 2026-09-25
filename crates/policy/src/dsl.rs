//! JSON parser for rule DSL v0.
//!
//! The parser is deliberately strict: unknown keys, absent required fields,
//! scalar type mismatches, and unsupported effect names are rejected.

use assistant_protocol::RiskLevel;
use assistant_protocol::serde_json::{Map, Value};

use crate::{
    Effect, EgressDestination, PolicyError, PolicyResult, PolicyRule, Reversibility,
    RuleConditions, RuleEffect, RuleId, RuleSet, ScopeOption,
};

pub fn parse_rule_set(json: &str) -> PolicyResult<RuleSet> {
    let value: Value = assistant_protocol::serde_json::from_str(json).map_err(|error| {
        PolicyError::InvalidRuleSet {
            reason: format!("invalid JSON: {error}"),
        }
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("rule set must be a JSON object"))?;
    reject_unknown_keys(object, &["default", "rules"], "rule set")?;
    parse_default(object.get("default"))?;

    let rules = match object.get("rules") {
        Some(Value::Array(values)) => values
            .iter()
            .enumerate()
            .map(|(index, value)| parse_rule(value, index))
            .collect::<PolicyResult<Vec<_>>>()?,
        Some(_) => return Err(invalid("'rules' must be an array")),
        None => Vec::new(),
    };
    Ok(RuleSet::new(rules))
}

fn parse_default(value: Option<&Value>) -> PolicyResult<()> {
    let object = value
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("'default' object is required"))?;
    reject_unknown_keys(object, &["effect"], "default")?;
    let effect = object
        .get("effect")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("default.effect must be a string"))?;
    if effect != "deny" {
        return Err(invalid("default.effect must be 'deny'"));
    }
    Ok(())
}

fn parse_rule(value: &Value, index: usize) -> PolicyResult<PolicyRule> {
    let context = format!("rules[{index}]");
    let object = value
        .as_object()
        .ok_or_else(|| invalid(&format!("{context} must be an object")))?;
    reject_unknown_keys(
        object,
        &["id", "when", "effect", "confirmation", "reason"],
        &context,
    )?;
    let id = required_string(object, "id", &context)?;
    if id.trim().is_empty() {
        return Err(invalid(&format!("{context}.id must not be empty")));
    }
    let conditions = parse_conditions(
        object
            .get("when")
            .ok_or_else(|| invalid(&format!("{context}.when is required")))?,
        &context,
    )?;
    let effect = parse_rule_effect(object, &context)?;
    Ok(PolicyRule {
        id: RuleId::new(id),
        conditions,
        effect,
    })
}

fn parse_rule_effect(object: &Map<String, Value>, context: &str) -> PolicyResult<RuleEffect> {
    let effect_name = required_string(object, "effect", context)?;
    match effect_name.as_str() {
        "allow" => {
            if object.contains_key("confirmation") || object.contains_key("reason") {
                return Err(invalid(&format!(
                    "{context} allow rule must not have confirmation or reason"
                )));
            }
            Ok(RuleEffect::Allow)
        }
        "allow_with_confirmation" => {
            if object.contains_key("reason") {
                return Err(invalid(&format!(
                    "{context} conditional allow must not have reason"
                )));
            }
            parse_confirmation(object, context)
        }
        "deny" => {
            if object.contains_key("confirmation") {
                return Err(invalid(&format!(
                    "{context} deny rule must not have confirmation"
                )));
            }
            let reason = required_string(object, "reason", context)?;
            Ok(RuleEffect::Deny { reason })
        }
        other => Err(invalid(&format!(
            "{context}.effect has unsupported value '{other}'"
        ))),
    }
}

fn parse_confirmation(object: &Map<String, Value>, context: &str) -> PolicyResult<RuleEffect> {
    let confirmation_context = format!("{context}.confirmation");
    let confirmation = object
        .get("confirmation")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid(&format!(
                "{confirmation_context} object is required for conditional allow"
            ))
        })?;
    reject_unknown_keys(
        confirmation,
        &["scope_options", "show_diff"],
        &confirmation_context,
    )?;
    let scope_options = parse_scope_options(
        confirmation
            .get("scope_options")
            .ok_or_else(|| invalid(&format!("{confirmation_context}.scope_options is required")))?,
        context,
    )?;
    let show_diff = confirmation
        .get("show_diff")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(&format!("{confirmation_context}.show_diff must be boolean")))?;
    Ok(RuleEffect::AllowWithConfirmation {
        scope_options,
        show_diff,
    })
}

fn parse_conditions(value: &Value, context: &str) -> PolicyResult<RuleConditions> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(&format!("{context}.when must be an object")))?;
    reject_unknown_keys(
        object,
        &[
            "effect",
            "risk_level",
            "reversibility",
            "unattended",
            "tainted",
            "target_app",
            "egress",
        ],
        &format!("{context}.when"),
    )?;
    Ok(RuleConditions {
        effect: parse_optional_enum_list(object.get("effect"), context, Effect::parse)?,
        risk_level: parse_optional_enum_list(object.get("risk_level"), context, RiskLevel::parse)?,
        reversibility: parse_optional_enum_list(
            object.get("reversibility"),
            context,
            Reversibility::parse,
        )?,
        unattended: parse_optional_bool(object.get("unattended"), context)?,
        tainted: parse_optional_bool(object.get("tainted"), context)?,
        target_app: parse_optional_string_list(object.get("target_app"), context)?,
        egress: parse_optional_enum_list(object.get("egress"), context, EgressDestination::parse)?,
    })
}

fn parse_optional_enum_list<T: Clone + PartialEq>(
    value: Option<&Value>,
    context: &str,
    parser: fn(&str) -> Option<T>,
) -> PolicyResult<Option<Vec<T>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let values = parse_string_list(value, context)?;
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let item = parser(&value).ok_or_else(|| {
            invalid(&format!(
                "{context}.when contains unsupported value '{value}'"
            ))
        })?;
        parsed.push(item);
    }
    Ok(Some(parsed))
}

fn parse_optional_string_list(
    value: Option<&Value>,
    context: &str,
) -> PolicyResult<Option<Vec<String>>> {
    value
        .map(|value| parse_string_list(value, context))
        .transpose()
}

fn parse_optional_bool(value: Option<&Value>, context: &str) -> PolicyResult<Option<bool>> {
    value
        .map(|value| {
            value.as_bool().ok_or_else(|| {
                invalid(&format!("{context}.when boolean condition must be boolean"))
            })
        })
        .transpose()
}

fn parse_string_list(value: &Value, context: &str) -> PolicyResult<Vec<String>> {
    match value {
        Value::String(item) => Ok(vec![item.clone()]),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| invalid(&format!("{context}.when list items must be strings")))
            })
            .collect(),
        _ => Err(invalid(&format!(
            "{context}.when condition must be a string or string array"
        ))),
    }
}

fn parse_scope_options(value: &Value, context: &str) -> PolicyResult<Vec<ScopeOption>> {
    let values = parse_string_list(value, context)?;
    values
        .into_iter()
        .map(|value| match value.as_str() {
            "once" => Ok(ScopeOption::Once),
            "this_task" => Ok(ScopeOption::ThisTask),
            other => Err(invalid(&format!(
                "{context}.confirmation.scope_options has unsupported value '{other}'"
            ))),
        })
        .collect()
}

fn required_string(object: &Map<String, Value>, key: &str, context: &str) -> PolicyResult<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid(&format!("{context}.{key} must be a string")))
}

fn reject_unknown_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
    context: &str,
) -> PolicyResult<()> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(invalid(&format!("{context} has unknown key '{key}'")));
        }
    }
    Ok(())
}

fn invalid(reason: &str) -> PolicyError {
    PolicyError::InvalidRuleSet {
        reason: reason.to_owned(),
    }
}

impl Effect {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            "send" => Some(Self::Send),
            "invoke" => Some(Self::Invoke),
            "destroy" => Some(Self::Destroy),
            _ => None,
        }
    }
}

impl Reversibility {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "L0_undo_stack" => Some(Self::L0UndoStack),
            "L1_snapshot" => Some(Self::L1Snapshot),
            "L2_compensating" => Some(Self::L2Compensating),
            "L3_irreversible" => Some(Self::L3Irreversible),
            _ => None,
        }
    }
}

impl EgressDestination {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "local_model" => Some(Self::LocalModel),
            "cloud_model" => Some(Self::CloudModel),
            _ => None,
        }
    }
}

trait ParseRiskLevel {
    fn parse(value: &str) -> Option<Self>
    where
        Self: Sized;
}

impl ParseRiskLevel for RiskLevel {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

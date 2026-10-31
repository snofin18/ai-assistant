//! Bounded, explicitly non-submitting browser form fill.
//!
//! The page expression is deliberately limited to text-like form controls. It
//! validates every selector and representation before writing any value, sets
//! native values, dispatches `input` / `change`, and never invokes submit,
//! requestSubmit, or click.

use std::collections::BTreeSet;
use std::time::Instant;

use assistant_platform_api::{CdpDomSnapshot, ErrorCode, PlatformError};
use assistant_protocol::serde_json;
use assistant_tool_bus::{CallContext, EvidenceDescriptor, ToolBusError, ToolHandler, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    BrowserCdpRuntime, BrowserHandlerContext, CDP_TARGET_FINGERPRINT_SCOPE, elapsed_millis,
    fingerprint_value, validate_http_url,
};
use crate::handler_support as support;

const MAX_FORM_FIELDS: usize = 100;
const MAX_SELECTOR_BYTES: usize = 4 * 1024;
const MAX_VALUE_BYTES: usize = 16 * 1024;
const MAX_TOTAL_VALUE_BYTES: usize = 64 * 1024;
const APPROVAL_READBACK_EVIDENCE_KEY: &str = "assistant.browser_form_fill.approval_readback.v1";

pub struct FormFillHandler<R> {
    pub(crate) context: std::sync::Arc<BrowserHandlerContext<R>>,
    pub(crate) tool_name: String,
}

#[derive(Debug, Clone)]
struct FieldSpec {
    selector: String,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FormFillPayload {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    filled_fields: Option<u64>,
    #[serde(default)]
    submitted: Option<bool>,
    #[serde(default)]
    form_origin: Option<String>,
    #[serde(default)]
    form_action: Option<String>,
    #[serde(default)]
    fields: Vec<FieldState>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldState {
    selector: String,
    requested: String,
    matched_by: FieldMatchKind,
    before: String,
    expected: String,
    after: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FieldMatchKind {
    Direct,
    Value,
    Text,
}

impl<R> ToolHandler for FormFillHandler<R>
where
    R: BrowserCdpRuntime + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &serde_json::Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let fields = parse_fields(arguments, &self.tool_name)?;
        let expression = form_fill_expression(&fields)?;
        let started = Instant::now();
        let before = self
            .context
            .runtime
            .evaluate_dom(&self.context.target, &expression)
            .map_err(|error| support::map_platform_error(&self.tool_name, &error))?;
        let payload: FormFillPayload =
            serde_json::from_str(before.dom()).map_err(|error| ToolBusError::Mcp {
                code: -32_000,
                message: format!(
                    "{}: form-fill DOM payload is not the required JSON object: {error}",
                    self.tool_name
                ),
            })?;
        let validated = validate_form_fill_payload(&fields, &payload, &self.tool_name)?;
        let previous_fingerprint = fingerprint_value(&json!({
            "cdp": cdp_identity(&before),
            "fields": payload.fields.iter().map(|state| json!({
                "selector": state.selector,
                "value": state.before,
            })).collect::<Vec<_>>(),
        }))?;
        let fingerprint = fingerprint_value(&json!({
            "cdp": cdp_identity(&before),
            "fields": payload.fields.iter().map(|state| json!({
                "selector": state.selector,
                "value": state.after,
            })).collect::<Vec<_>>(),
        }))?;
        Ok(ToolOutput::json(json!({
            "filled_fields": validated.filled_fields,
            "submitted": false,
            "fingerprint_scope": CDP_TARGET_FINGERPRINT_SCOPE,
            "fingerprint": fingerprint,
            "previous_fingerprint": previous_fingerprint,
            "elapsed_ms": elapsed_millis(started.elapsed()),
        }))
        .with_evidence(EvidenceDescriptor {
            extra: Some(approval_readback_evidence(&payload, &validated)),
            ..EvidenceDescriptor::default()
        }))
    }
}

struct ValidatedFormFill<'payload> {
    filled_fields: u64,
    form_origin: &'payload str,
    form_action: &'payload str,
}

fn parse_fields(
    arguments: &serde_json::Map<String, Value>,
    tool: &str,
) -> Result<Vec<FieldSpec>, ToolBusError> {
    let values = arguments
        .get("fields")
        .and_then(Value::as_array)
        .ok_or_else(|| support::invalid_arguments(tool, "fields must be an array"))?;
    if values.is_empty() || values.len() > MAX_FORM_FIELDS {
        return Err(support::invalid_arguments(
            tool,
            format!("fields must contain between 1 and {MAX_FORM_FIELDS} entries"),
        ));
    }
    let mut selectors = BTreeSet::new();
    let mut total_value_bytes = 0usize;
    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        let field = parse_field(value, tool)?;
        if !selectors.insert(field.selector.clone()) {
            return Err(support::invalid_arguments(
                tool,
                format!("field selector `{}` appears more than once", field.selector),
            ));
        }
        total_value_bytes = total_value_bytes.saturating_add(field.value.len());
        if total_value_bytes > MAX_TOTAL_VALUE_BYTES {
            return Err(support::invalid_arguments(
                tool,
                "total form value bytes exceed 64 KiB",
            ));
        }
        fields.push(field);
    }
    Ok(fields)
}

fn parse_field(value: &Value, tool: &str) -> Result<FieldSpec, ToolBusError> {
    let object = value
        .as_object()
        .ok_or_else(|| support::invalid_arguments(tool, "each form field must be an object"))?;
    let selector = object
        .get("selector")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| support::invalid_arguments(tool, "field selector must be non-empty"))?;
    let value = object
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| support::invalid_arguments(tool, "field value must be a string"))?;
    if selector.len() > MAX_SELECTOR_BYTES {
        return Err(support::invalid_arguments(
            tool,
            "field selector exceeds 4 KiB",
        ));
    }
    if value.len() > MAX_VALUE_BYTES {
        return Err(support::invalid_arguments(
            tool,
            "field value exceeds 16 KiB",
        ));
    }
    if has_forbidden_control(selector) || has_forbidden_control(value) {
        return Err(support::invalid_arguments(
            tool,
            "selector and value must not contain NUL or unsupported controls",
        ));
    }
    Ok(FieldSpec {
        selector: selector.to_owned(),
        value: value.to_owned(),
    })
}

fn validate_form_fill_payload<'payload>(
    fields: &[FieldSpec],
    payload: &'payload FormFillPayload,
    tool: &str,
) -> Result<ValidatedFormFill<'payload>, ToolBusError> {
    if let Some(error) = payload.error.as_deref() {
        let code = match error {
            "not_found" => ErrorCode::TargetNotFound,
            "ambiguous" | "no_option" => ErrorCode::TargetAmbiguous,
            "unsupported" | "forbidden" | "invalid_selector" | "no_form" | "cross_form" => {
                ErrorCode::ToolInvalidArgs
            }
            _ => ErrorCode::VerifyFailed,
        };
        return Err(support::map_platform_error(
            tool,
            &PlatformError::new(
                code,
                payload.message.as_ref().map_or_else(
                    || {
                        payload.selector.as_deref().map_or_else(
                            || format!("form fill failed: {error}"),
                            |selector| format!("form fill failed for `{selector}`: {error}"),
                        )
                    },
                    Clone::clone,
                ),
            ),
        ));
    }
    if payload.submitted != Some(false) {
        return Err(ToolBusError::Mcp {
            code: -32_000,
            message: format!("{tool}: form fill must return submitted=false"),
        });
    }
    let filled_fields = payload.filled_fields.unwrap_or_default();
    if filled_fields != u64::try_from(fields.len()).unwrap_or(u64::MAX) {
        return Err(ToolBusError::Mcp {
            code: -32_000,
            message: format!(
                "{tool}: form fill reported {filled_fields} fields, expected {}",
                fields.len()
            ),
        });
    }
    let form_origin = payload
        .form_origin
        .as_deref()
        .ok_or_else(|| invalid_form_fill_result(tool, "form fill returned no form origin"))?;
    let form_action = payload
        .form_action
        .as_deref()
        .ok_or_else(|| invalid_form_fill_result(tool, "form fill returned no form action"))?;
    validate_form_origin(form_origin, tool)?;
    validate_form_action(form_action, tool)?;
    validate_field_states(fields, &payload.fields, tool)?;
    Ok(ValidatedFormFill {
        filled_fields,
        form_origin,
        form_action,
    })
}

fn approval_readback_evidence(
    payload: &FormFillPayload,
    validated: &ValidatedFormFill<'_>,
) -> Value {
    let readback = json!({
        "form_origin": validated.form_origin,
        "form_action": validated.form_action,
        "fields": payload.fields.iter().map(|state| json!({
            "selector": state.selector,
            "before": state.before,
            "after": state.after,
        })).collect::<Vec<_>>(),
    });
    Value::Object(serde_json::Map::from_iter([(
        APPROVAL_READBACK_EVIDENCE_KEY.to_owned(),
        readback,
    )]))
}

fn has_forbidden_control(value: &str) -> bool {
    value.chars().any(|character| {
        character == '\0' || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    })
}

fn form_fill_expression(fields: &[FieldSpec]) -> Result<String, ToolBusError> {
    let field_json = serialize_field_specs(fields)?;
    Ok(format!(
        "(() => {{ const specs = {field_json}; \
         const allowed = new Set(['text','search','email','url','tel','number','date','time',\
         'datetime-local','month','week']); \
         const read = (element) => element instanceof HTMLSelectElement ? element.value : \
         element.isContentEditable ? (element.textContent || '') : element.value; \
         const resolve = (spec) => {{ let nodes; \
           try {{ nodes = document.querySelectorAll(spec.selector); }} \
           catch (_) {{ return {{ error:'invalid_selector', selector:spec.selector }}; }} \
           if (!nodes || nodes.length === 0) return {{ error:'not_found', selector:spec.selector }}; \
           if (nodes.length !== 1) return {{ error:'ambiguous', selector:spec.selector }}; \
           const element = nodes[0]; let expected = spec.value; let matchedBy = 'direct'; \
           if (element instanceof HTMLInputElement) {{ \
             const kind = (element.type || 'text').toLowerCase(); \
             if (!allowed.has(kind)) return {{ error:'forbidden', selector:spec.selector }}; \
           }} else if (element instanceof HTMLTextAreaElement) {{ \
           }} else if (element instanceof HTMLSelectElement) {{ \
             const options = Array.from(element.options); \
             const valueOption = options.find((item) => item.value === spec.value); \
             const textOption = options.find((item) => item.textContent === spec.value); \
             const option = valueOption || textOption; \
             if (!option) return {{ error:'no_option', selector:spec.selector }}; \
             expected = option.value; matchedBy = valueOption ? 'value' : 'text'; \
           }} else if (!element.isContentEditable) {{ \
             return {{ error:'unsupported', selector:spec.selector }}; \
           }} \
           const form = element.form instanceof HTMLFormElement ? element.form : \
             element.closest('form'); \
           if (!(form instanceof HTMLFormElement)) {{ \
             return {{ error:'no_form', selector:spec.selector }}; \
           }} \
           return {{ element, form, selector:spec.selector, requested:spec.value, \
             matched_by:matchedBy, expected }}; }}; \
         const resolved = specs.map(resolve); \
         const failed = resolved.find((item) => item.error); \
         if (failed) return JSON.stringify(failed); \
         const form = resolved[0].form; \
         if (resolved.some((item) => item.form !== form)) {{ \
           return JSON.stringify({{ error:'cross_form' }}); \
         }} \
         let actionUrl; \
         try {{ actionUrl = new URL(form.getAttribute('action') || location.href, location.href); }} \
         catch (_) {{ return JSON.stringify({{ error:'invalid_action' }}); }} \
         if (!['http:','https:'].includes(location.protocol) || \
             !['http:','https:'].includes(actionUrl.protocol) || \
             actionUrl.username || actionUrl.password || actionUrl.hash) {{ \
           return JSON.stringify({{ error:'invalid_action' }}); \
         }} \
         const action = actionUrl.href; \
         const states = resolved.map((item) => ({{
           element:item.element, selector:item.selector, requested:item.requested,
           matched_by:item.matched_by, expected:item.expected, before:read(item.element),
         }})); \
         try {{ for (const state of states) {{ \
           if (state.element.isContentEditable) state.element.textContent = state.expected; \
           else {{ \
             const descriptor = Object.getOwnPropertyDescriptor(
               Object.getPrototypeOf(state.element), 'value'
             ); \
             if (descriptor && descriptor.set) descriptor.set.call(state.element, state.expected); \
             else state.element.value = state.expected; \
           }} \
           state.after = read(state.element); \
           state.element.dispatchEvent(new Event('input', {{ bubbles:true }})); \
           state.element.dispatchEvent(new Event('change', {{ bubbles:true }})); \
         }} }} catch (_) {{ return JSON.stringify({{ error:'write_failed' }}); }} \
         return JSON.stringify({{ filled_fields:states.length, submitted:false, \
           form_origin:location.origin, form_action:action, \
           fields:states.map((state) => ({{ selector:state.selector, requested:state.requested, \
             matched_by:state.matched_by, before:state.before, expected:state.expected, \
             after:state.after }})) }}); }})()"
    ))
}

fn serialize_field_specs(fields: &[FieldSpec]) -> Result<String, ToolBusError> {
    serde_json::to_string(
        &fields
            .iter()
            .map(|field| json!({ "selector": field.selector, "value": field.value }))
            .collect::<Vec<_>>(),
    )
    .map_err(|error| ToolBusError::Mcp {
        code: -32_000,
        message: format!("form field serialization failed: {error}"),
    })
}

fn validate_field_states(
    fields: &[FieldSpec],
    states: &[FieldState],
    tool: &str,
) -> Result<(), ToolBusError> {
    if states.len() != fields.len() {
        return Err(invalid_form_fill_result(
            tool,
            "form fill did not return every requested field",
        ));
    }
    let mut total_value_bytes = 0usize;
    for (field, state) in fields.iter().zip(states) {
        if state.selector != field.selector {
            return Err(invalid_form_fill_result(
                tool,
                "form fill returned a field that does not match the request",
            ));
        }
        if state.requested != field.value {
            return Err(invalid_form_fill_result(
                tool,
                "form fill returned a field that does not match the request",
            ));
        }
        if state.selector.len() > MAX_SELECTOR_BYTES || has_forbidden_control(&state.selector) {
            return Err(invalid_form_fill_result(
                tool,
                "form fill returned an invalid field selector",
            ));
        }
        for value in [
            state.requested.as_str(),
            state.expected.as_str(),
            state.before.as_str(),
            state.after.as_str(),
        ] {
            if value.len() > MAX_VALUE_BYTES || has_forbidden_control(value) {
                return Err(invalid_form_fill_result(
                    tool,
                    "form fill returned a field value outside its safe bound",
                ));
            }
        }
        total_value_bytes = total_value_bytes
            .saturating_add(state.before.len())
            .saturating_add(state.after.len());
        if total_value_bytes > MAX_TOTAL_VALUE_BYTES {
            return Err(invalid_form_fill_result(
                tool,
                "form fill readback exceeds 64 KiB",
            ));
        }
        let expected_matches = match state.matched_by {
            FieldMatchKind::Direct | FieldMatchKind::Value => state.expected == field.value,
            FieldMatchKind::Text => true,
        };
        if !expected_matches || state.after != state.expected {
            return Err(invalid_form_fill_result(
                tool,
                "form fill did not read back every requested value",
            ));
        }
    }
    Ok(())
}

fn invalid_form_fill_result(tool: &str, reason: &str) -> ToolBusError {
    ToolBusError::Mcp {
        code: -32_000,
        message: format!("{tool}: {reason}"),
    }
}

fn validate_form_origin(origin: &str, tool: &str) -> Result<(), ToolBusError> {
    validate_http_url(origin, tool)?;
    if http_origin_prefix(origin) != Some(origin) {
        return Err(invalid_form_fill_result(
            tool,
            "form fill returned a form origin with a path or userinfo",
        ));
    }
    Ok(())
}

fn validate_form_action(action: &str, tool: &str) -> Result<(), ToolBusError> {
    validate_http_url(action, tool)?;
    if action.contains('#') || http_origin_prefix(action).is_none() {
        return Err(invalid_form_fill_result(
            tool,
            "form fill returned an unsafe form action",
        ));
    }
    Ok(())
}

fn http_origin_prefix(url: &str) -> Option<&str> {
    let scheme_end = url.find("://")?;
    let scheme = url.get(..scheme_end)?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority_start = scheme_end.checked_add(3)?;
    let remainder = url.get(authority_start..)?;
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = remainder.get(..authority_end)?;
    if authority.is_empty() || authority.contains('@') || authority.contains('\\') {
        return None;
    }
    url.get(..authority_start.checked_add(authority_end)?)
}

fn cdp_identity(snapshot: &CdpDomSnapshot) -> Value {
    json!({
        "url": snapshot.target_url(),
        "title": snapshot.target_title(),
        "marker": snapshot.target_marker(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    fn field(selector: &str, value: &str) -> FieldSpec {
        FieldSpec {
            selector: selector.to_owned(),
            value: value.to_owned(),
        }
    }

    #[test]
    fn test_form_fill_expression_has_no_submit_or_click() {
        let result = form_fill_expression(&[field("#name", "Ada")]);
        assert!(result.is_ok());
        if let Ok(expression) = result {
            assert!(!expression.contains(".submit("));
            assert!(!expression.contains(".requestSubmit("));
            assert!(!expression.contains(".click("));
            assert!(expression.contains("submitted:false"));
            assert!(expression.contains("HTMLSelectElement"));
            assert!(expression.contains("fields:states.map"));
            assert!(!expression.contains("fields:states }}"));
            assert!(expression.contains("cross_form"));
            assert!(expression.contains("no_form"));
            assert!(expression.contains("form_origin:location.origin"));
            assert!(expression.contains("form_action:action"));
        }
    }

    #[test]
    fn test_form_fill_approval_evidence_contains_only_readback_fields() {
        let payload = FormFillPayload {
            error: None,
            selector: None,
            message: None,
            filled_fields: Some(1),
            submitted: Some(false),
            form_origin: Some("http://127.0.0.1:8765".to_owned()),
            form_action: Some("http://127.0.0.1:8765/submitted".to_owned()),
            fields: vec![FieldState {
                selector: "#name".to_owned(),
                requested: "Ada".to_owned(),
                matched_by: FieldMatchKind::Direct,
                before: String::new(),
                expected: "Ada".to_owned(),
                after: "Ada".to_owned(),
            }],
        };
        let validated = ValidatedFormFill {
            filled_fields: 1,
            form_origin: "http://127.0.0.1:8765",
            form_action: "http://127.0.0.1:8765/submitted",
        };
        let evidence = approval_readback_evidence(&payload, &validated);
        let readback = &evidence[APPROVAL_READBACK_EVIDENCE_KEY];
        assert_eq!(readback["form_origin"], "http://127.0.0.1:8765");
        assert_eq!(readback["form_action"], "http://127.0.0.1:8765/submitted");
        assert_eq!(readback["fields"][0]["selector"], "#name");
        assert_eq!(readback["fields"][0]["before"], "");
        assert_eq!(readback["fields"][0]["after"], "Ada");
        assert!(readback["fields"][0].get("requested").is_none());
        assert!(readback["fields"][0].get("matched_by").is_none());
        assert!(readback["fields"][0].get("expected").is_none());
    }

    #[test]
    fn test_form_fill_payload_requires_http_origin_and_action() {
        let fields = vec![field("#name", "Ada")];
        let mut payload = FormFillPayload {
            error: None,
            selector: None,
            message: None,
            filled_fields: Some(1),
            submitted: Some(false),
            form_origin: Some("file:///tmp/form".to_owned()),
            form_action: Some("http://127.0.0.1:8765/submitted".to_owned()),
            fields: vec![FieldState {
                selector: "#name".to_owned(),
                requested: "Ada".to_owned(),
                matched_by: FieldMatchKind::Direct,
                before: String::new(),
                expected: "Ada".to_owned(),
                after: "Ada".to_owned(),
            }],
        };
        assert!(validate_form_fill_payload(&fields, &payload, "edge.cdp.form_fill").is_err());
        payload.form_origin = Some("http://127.0.0.1:8765".to_owned());
        payload.form_action = Some("javascript:alert(1)".to_owned());
        assert!(validate_form_fill_payload(&fields, &payload, "edge.cdp.form_fill").is_err());
        payload.form_action = Some("http://user:secret@127.0.0.1:8765/submitted".to_owned());
        assert!(validate_form_fill_payload(&fields, &payload, "edge.cdp.form_fill").is_err());
        payload.form_action = Some("http://127.0.0.1:8765/submitted#fragment".to_owned());
        assert!(validate_form_fill_payload(&fields, &payload, "edge.cdp.form_fill").is_err());
    }

    #[test]
    fn test_form_fill_readback_rejects_values_over_fixed_bound() {
        let fields = vec![field("#name", "Ada")];
        let states = vec![FieldState {
            selector: "#name".to_owned(),
            requested: "Ada".to_owned(),
            matched_by: FieldMatchKind::Direct,
            before: String::new(),
            expected: "Ada".to_owned(),
            after: "x".repeat(MAX_VALUE_BYTES + 1),
        }];
        assert!(validate_field_states(&fields, &states, "edge.cdp.form_fill").is_err());
    }

    #[test]
    fn test_form_fill_expression_rejects_duplicate_selectors() {
        let fields = [field("#name", "Ada"), field("#name", "Grace")];
        let map = serde_json::Map::from_iter([(
            "fields".to_owned(),
            Value::Array(
                fields
                    .iter()
                    .map(|field| json!({"selector": field.selector, "value": field.value}))
                    .collect(),
            ),
        )]);
        let result = parse_fields(&map, "edge.cdp.form_fill");
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(matches!(error, ToolBusError::InvalidArguments { .. }));
        }
    }

    #[test]
    fn test_form_fill_expression_rejects_forbidden_controls() {
        let map = serde_json::Map::from_iter([(
            "fields".to_owned(),
            json!([{"selector": "#name", "value": "bad\u{0000}value"}]),
        )]);
        let result = parse_fields(&map, "edge.cdp.form_fill");
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(matches!(error, ToolBusError::InvalidArguments { .. }));
        }
    }

    #[test]
    fn test_form_fill_error_payload_includes_selector() {
        let result: Result<FormFillPayload, _> =
            serde_json::from_str(r##"{"error":"not_found","selector":"#missing"}"##);
        assert!(result.is_ok());
        if let Ok(payload) = result {
            assert_eq!(payload.error.as_deref(), Some("not_found"));
            assert_eq!(payload.selector.as_deref(), Some("#missing"));
        }
    }

    #[test]
    fn test_form_fill_state_validation_rejects_unrequested_value() {
        let fields = vec![field("#name", "Ada")];
        let states = vec![FieldState {
            selector: "#name".to_owned(),
            requested: "Grace".to_owned(),
            matched_by: FieldMatchKind::Direct,
            before: String::new(),
            expected: "Grace".to_owned(),
            after: "Grace".to_owned(),
        }];
        let result = validate_field_states(&fields, &states, "edge.cdp.form_fill");
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(matches!(error, ToolBusError::Mcp { code: -32_000, .. }));
        }
    }

    #[test]
    fn test_form_fill_state_validation_accepts_select_text_match() {
        let fields = vec![field("#country", "China")];
        let states = vec![FieldState {
            selector: "#country".to_owned(),
            requested: "China".to_owned(),
            matched_by: FieldMatchKind::Text,
            before: String::new(),
            expected: "CN".to_owned(),
            after: "CN".to_owned(),
        }];
        assert!(validate_field_states(&fields, &states, "edge.cdp.form_fill").is_ok());
    }
}

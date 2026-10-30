//! Dedicated, same-origin form submission handler.
//!
//! The DOM expression validates the form, submit control, URL and exact field
//! state before one click. A second read must observe a new navigation time
//! origin; any unknown post-click outcome returns `UserInteraction`.

use std::collections::BTreeSet;
use std::sync::Arc;

use assistant_platform_api::{ErrorCode, PlatformError};
use assistant_protocol::serde_json;
use assistant_tool_bus::{CallContext, SourceDescriptor, ToolBusError, ToolHandler, ToolOutput};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::{BrowserCdpRuntime, BrowserHandlerContext};
use crate::handler_support as support;

const MAX_FORM_FIELDS: usize = 100;
const MAX_SELECTOR_BYTES: usize = 4 * 1024;
const MAX_VALUE_BYTES: usize = 16 * 1024;
const MAX_TOTAL_VALUE_BYTES: usize = 64 * 1024;
const MAX_URL_BYTES: usize = 8 * 1024;
const SUBMIT_QUIET_MS: u64 = 500;
const POST_SUBMIT_IDENTITY_EXPRESSION: &str =
    "JSON.stringify({ url: location.href, time_origin: performance.timeOrigin })";

pub struct FormSubmitHandler<R> {
    pub(crate) context: Arc<BrowserHandlerContext<R>>,
    pub(crate) tool_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedField {
    selector: String,
    value: String,
}

struct SubmitRequest {
    form_selector: String,
    submit_selector: String,
    expected_origin: String,
    expected_action: String,
    fields: Vec<ExpectedField>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SubmitPayload {
    error: Option<String>,
    selector: Option<String>,
    clicked: Option<bool>,
    pre_time_origin: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PostSubmitPayload {
    url: Option<String>,
    time_origin: Option<f64>,
}

impl<R> ToolHandler for FormSubmitHandler<R>
where
    R: BrowserCdpRuntime + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let request = parse_submit_request(arguments, &self.tool_name)?;
        let expression = form_submit_expression(
            &request.form_selector,
            &request.submit_selector,
            &request.expected_origin,
            &request.expected_action,
            &request.fields,
        )?;
        let preflight = self.evaluate_preflight(&expression)?;
        let pre_time_origin = validate_preflight(&preflight, &self.tool_name)?;
        let post_payload = self.read_post_submit_identity()?;
        let post_url = validate_post_submit(
            &post_payload,
            &request.expected_origin,
            pre_time_origin,
            &self.tool_name,
        )?;
        Ok(ToolOutput::untrusted(
            SourceDescriptor::app_content(self.context.app_id.clone()).with_target("browser_form"),
            json!({"submitted": true, "post_url": post_url}),
        ))
    }
}

impl<R> FormSubmitHandler<R>
where
    R: BrowserCdpRuntime + 'static,
{
    fn evaluate_preflight(&self, expression: &str) -> Result<SubmitPayload, ToolBusError> {
        let snapshot = self
            .context
            .runtime
            .evaluate_dom(&self.context.target, expression)
            .map_err(|error| {
                unknown_outcome(
                    &self.tool_name,
                    &format!("pre-click evaluation failed: {error}"),
                )
            })?;
        parse_submit_payload(snapshot.dom(), &self.tool_name)
    }

    fn read_post_submit_identity(&self) -> Result<PostSubmitPayload, ToolBusError> {
        self.context
            .runtime
            .wait_network_idle(&self.context.target, SUBMIT_QUIET_MS)
            .map_err(|error| {
                unknown_outcome(
                    &self.tool_name,
                    &format!("post-click network wait failed: {error}"),
                )
            })?;
        let snapshot = self
            .context
            .runtime
            .evaluate_dom(&self.context.target, POST_SUBMIT_IDENTITY_EXPRESSION)
            .map_err(|error| {
                unknown_outcome(
                    &self.tool_name,
                    &format!("post-click identity read failed: {error}"),
                )
            })?;
        parse_post_submit_payload(snapshot.dom(), &self.tool_name)
    }
}

fn parse_submit_request(
    arguments: &Map<String, Value>,
    tool: &str,
) -> Result<SubmitRequest, ToolBusError> {
    let form_selector = required_text(arguments, "form_selector", tool)?;
    let submit_selector = required_text(arguments, "submit_selector", tool)?;
    let expected_origin = required_text(arguments, "expected_origin", tool)?;
    let expected_action = required_text(arguments, "expected_action", tool)?;
    validate_origin(expected_origin, tool)?;
    validate_action(expected_action, expected_origin, tool)?;
    let fields = parse_fields(arguments, tool)?;
    Ok(SubmitRequest {
        form_selector: form_selector.to_owned(),
        submit_selector: submit_selector.to_owned(),
        expected_origin: expected_origin.to_owned(),
        expected_action: expected_action.to_owned(),
        fields,
    })
}

fn parse_fields(
    arguments: &Map<String, Value>,
    tool: &str,
) -> Result<Vec<ExpectedField>, ToolBusError> {
    let values = arguments
        .get("expected_fields")
        .and_then(Value::as_array)
        .ok_or_else(|| support::invalid_arguments(tool, "expected_fields must be an array"))?;
    if values.is_empty() || values.len() > MAX_FORM_FIELDS {
        return Err(support::invalid_arguments(
            tool,
            "invalid expected_fields count",
        ));
    }
    let mut selectors = BTreeSet::new();
    let mut total = 0usize;
    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        let field = parse_expected_field(value, tool)?;
        if !selectors.insert(field.selector.clone()) {
            return Err(support::invalid_arguments(tool, "duplicate field selector"));
        }
        total = total.saturating_add(field.value.len());
        if total > MAX_TOTAL_VALUE_BYTES {
            return Err(support::invalid_arguments(
                tool,
                "expected field values exceed 64 KiB",
            ));
        }
        fields.push(field);
    }
    Ok(fields)
}

fn parse_expected_field(value: &Value, tool: &str) -> Result<ExpectedField, ToolBusError> {
    let object = value
        .as_object()
        .ok_or_else(|| support::invalid_arguments(tool, "field must be an object"))?;
    if object.len() != 2 {
        return Err(support::invalid_arguments(tool, "field keys"));
    }
    let selector = object
        .get("selector")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| support::invalid_arguments(tool, "field selector must be non-empty"))?;
    let value = object
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| support::invalid_arguments(tool, "field value must be a string"))?;
    if selector.len() > MAX_SELECTOR_BYTES
        || value.len() > MAX_VALUE_BYTES
        || has_forbidden_control(selector)
        || has_forbidden_control(value)
    {
        return Err(support::invalid_arguments(tool, "invalid field bounds"));
    }
    Ok(ExpectedField {
        selector: selector.to_owned(),
        value: value.to_owned(),
    })
}

fn validate_preflight(payload: &SubmitPayload, tool: &str) -> Result<f64, ToolBusError> {
    if let Some(error) = payload.error.as_deref() {
        if error == "click_failed" {
            return Err(unknown_outcome(tool, "submit click threw"));
        }
        let code = match error {
            "not_found" => ErrorCode::TargetNotFound,
            "ambiguous" => ErrorCode::TargetAmbiguous,
            "invalid_selector"
            | "invalid_action"
            | "unsupported"
            | "forbidden"
            | "too_large"
            | "action_override"
            | "submit_not_owned"
            | "validation_disabled"
            | "validation_override" => ErrorCode::ToolInvalidArgs,
            _ => ErrorCode::VerifyFailed,
        };
        let subject = payload.selector.as_deref().unwrap_or("form");
        return Err(support::map_platform_error(
            tool,
            &PlatformError::new(code, format!("form submit failed for `{subject}`: {error}")),
        ));
    }
    if payload.clicked != Some(true) {
        return Err(unknown_outcome(
            tool,
            "pre-click payload did not report clicked=true",
        ));
    }
    payload
        .pre_time_origin
        .ok_or_else(|| unknown_outcome(tool, "pre-click payload has no navigation marker"))
}

fn parse_submit_payload(input: &str, tool: &str) -> Result<SubmitPayload, ToolBusError> {
    serde_json::from_str(input)
        .map_err(|error| unknown_outcome(tool, &format!("pre-click payload is not JSON: {error}")))
}

fn parse_post_submit_payload(input: &str, tool: &str) -> Result<PostSubmitPayload, ToolBusError> {
    serde_json::from_str(input)
        .map_err(|error| unknown_outcome(tool, &format!("post-click payload is not JSON: {error}")))
}

fn validate_post_submit<'a>(
    payload: &'a PostSubmitPayload,
    expected_origin: &str,
    pre_time_origin: f64,
    tool: &str,
) -> Result<&'a str, ToolBusError> {
    let post_url = payload
        .url
        .as_deref()
        .ok_or_else(|| unknown_outcome(tool, "post-click payload has no URL"))?;
    if !is_safe_http_url(post_url, Some(expected_origin)) {
        return Err(unknown_outcome(
            tool,
            "post-click URL is not a safe same-origin HTTP(S) URL",
        ));
    }
    let post_time_origin = payload
        .time_origin
        .ok_or_else(|| unknown_outcome(tool, "post-click payload has no navigation marker"))?;
    if !pre_time_origin.is_finite()
        || !post_time_origin.is_finite()
        || (post_time_origin - pre_time_origin).abs() <= 0.001
    {
        return Err(unknown_outcome(
            tool,
            "post-click navigation marker did not change",
        ));
    }
    Ok(post_url)
}

fn form_submit_expression(
    form_selector: &str,
    submit_selector: &str,
    expected_origin: &str,
    expected_action: &str,
    fields: &[ExpectedField],
) -> Result<String, ToolBusError> {
    let form_selector =
        serde_json::to_string(form_selector).map_err(|error| serialization_error(&error))?;
    let submit_selector =
        serde_json::to_string(submit_selector).map_err(|error| serialization_error(&error))?;
    let expected_origin =
        serde_json::to_string(expected_origin).map_err(|error| serialization_error(&error))?;
    let expected_action =
        serde_json::to_string(expected_action).map_err(|error| serialization_error(&error))?;
    let fields = serde_json::to_string(fields).map_err(|error| serialization_error(&error))?;
    Ok(format!(
        "(() => {{ const formSelector={form_selector}; const submitSelector={submit_selector}; const expectedOrigin={expected_origin}; const expectedAction={expected_action}; const expectedFields={fields}; const allowed=new Set(['text','search','email','url','tel','number','date','time','datetime-local','month','week']); const read=(e)=>e instanceof HTMLSelectElement?e.value:e.isContentEditable?(e.textContent||''):e.value; let forms; try {{ forms=document.querySelectorAll(formSelector); }} catch(_) {{ return JSON.stringify({{error:'invalid_selector',selector:formSelector}}); }} if(!forms||forms.length===0)return JSON.stringify({{error:'not_found',selector:formSelector}}); if(forms.length!==1)return JSON.stringify({{error:'ambiguous',selector:formSelector}}); const form=forms[0]; if(!(form instanceof HTMLFormElement))return JSON.stringify({{error:'unsupported',selector:formSelector}}); if(form.noValidate)return JSON.stringify({{error:'validation_disabled',selector:formSelector}}); if((form.method||'get').toLowerCase()==='dialog')return JSON.stringify({{error:'unsupported',selector:formSelector}}); const formTarget=(form.getAttribute('target')||'').toLowerCase(); if(formTarget&&formTarget!=='_self')return JSON.stringify({{error:'target_unsafe',selector:formSelector}}); for(const input of form.querySelectorAll('input')) {{ const kind=(input.type||'text').toLowerCase(); if(kind==='password'||kind==='file')return JSON.stringify({{error:'forbidden',selector:formSelector}}); }} let submits; try {{ submits=form.querySelectorAll(submitSelector); }} catch(_) {{ return JSON.stringify({{error:'invalid_selector',selector:submitSelector}}); }} if(!submits||submits.length===0)return JSON.stringify({{error:'not_found',selector:submitSelector}}); if(submits.length!==1)return JSON.stringify({{error:'ambiguous',selector:submitSelector}}); const submit=submits[0]; if(!(submit instanceof HTMLButtonElement)&&!(submit instanceof HTMLInputElement))return JSON.stringify({{error:'unsupported',selector:submitSelector}}); if(submit.form!==form)return JSON.stringify({{error:'submit_not_owned',selector:submitSelector}}); if((submit.getAttribute('type')||'').toLowerCase()!=='submit'||submit.disabled)return JSON.stringify({{error:'forbidden',selector:submitSelector}}); if(submit.hasAttribute('formnovalidate'))return JSON.stringify({{error:'validation_override',selector:submitSelector}}); if(submit.hasAttribute('formaction'))return JSON.stringify({{error:'action_override',selector:submitSelector}}); const submitTarget=(submit.getAttribute('formtarget')||'').toLowerCase(); if(submitTarget&&submitTarget!=='_self')return JSON.stringify({{error:'target_unsafe',selector:submitSelector}}); if(location.origin!==expectedOrigin)return JSON.stringify({{error:'origin_mismatch',selector:formSelector}}); let actionUrl; try {{ actionUrl=new URL(form.getAttribute('action')||location.href,location.href); }} catch(_) {{ return JSON.stringify({{error:'invalid_action',selector:formSelector}}); }} if(!['http:','https:'].includes(actionUrl.protocol)||actionUrl.username||actionUrl.password||actionUrl.origin!==expectedOrigin||actionUrl.href!==expectedAction)return JSON.stringify({{error:'action_mismatch',selector:formSelector}}); const encoder=new TextEncoder(); let total=0; for(const spec of expectedFields) {{ let nodes; try {{ nodes=form.querySelectorAll(spec.selector); }} catch(_) {{ return JSON.stringify({{error:'invalid_selector',selector:spec.selector}}); }} if(!nodes||nodes.length===0)return JSON.stringify({{error:'not_found',selector:spec.selector}}); if(nodes.length!==1)return JSON.stringify({{error:'ambiguous',selector:spec.selector}}); const e=nodes[0]; if(e instanceof HTMLInputElement) {{ const kind=(e.type||'text').toLowerCase(); if(!allowed.has(kind))return JSON.stringify({{error:'forbidden',selector:spec.selector}}); }} else if(!(e instanceof HTMLTextAreaElement)&&!(e instanceof HTMLSelectElement)&&!e.isContentEditable) {{ return JSON.stringify({{error:'unsupported',selector:spec.selector}}); }} const value=String(read(e)); if(value!==spec.value)return JSON.stringify({{error:'field_mismatch',selector:spec.selector}}); const bytes=encoder.encode(value).length; total+=bytes; if(bytes>{MAX_VALUE_BYTES}||total>{MAX_TOTAL_VALUE_BYTES})return JSON.stringify({{error:'too_large',selector:spec.selector}}); }} if(!form.checkValidity())return JSON.stringify({{error:'validation_failed',selector:formSelector}}); const preTimeOrigin=performance.timeOrigin; try {{ submit.click(); }} catch(_) {{ return JSON.stringify({{error:'click_failed',selector:submitSelector}}); }} return JSON.stringify({{clicked:true,pre_time_origin:preTimeOrigin}}); }})()"
    ))
}

fn required_text<'a>(
    arguments: &'a Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<&'a str, ToolBusError> {
    let value = arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| support::invalid_arguments(tool, format!("{field} must be non-empty")))?;
    if value.len() > MAX_SELECTOR_BYTES || has_forbidden_control(value) {
        return Err(support::invalid_arguments(tool, format!("invalid {field}")));
    }
    Ok(value)
}

fn validate_origin(origin: &str, tool: &str) -> Result<(), ToolBusError> {
    if origin.len() > MAX_URL_BYTES || http_origin_prefix(origin) != Some(origin) {
        return Err(support::invalid_arguments(
            tool,
            "expected_origin must be an HTTP(S) origin",
        ));
    }
    Ok(())
}

fn validate_action(action: &str, expected_origin: &str, tool: &str) -> Result<(), ToolBusError> {
    if !is_safe_http_url(action, Some(expected_origin)) || action.contains('#') {
        return Err(support::invalid_arguments(
            tool,
            "expected_action must be a same-origin HTTP(S) URL without credentials or fragment",
        ));
    }
    Ok(())
}

fn is_safe_http_url(url: &str, expected_origin: Option<&str>) -> bool {
    !url.is_empty()
        && url.len() <= MAX_URL_BYTES
        && !url.contains('#')
        && !url.contains('\\')
        && !url
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        && http_origin_prefix(url)
            .is_some_and(|origin| expected_origin.is_none_or(|expected| origin == expected))
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

fn has_forbidden_control(value: &str) -> bool {
    value.chars().any(|character| {
        character == '\0' || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    })
}

fn unknown_outcome(tool: &str, reason: &str) -> ToolBusError {
    ToolBusError::Mcp {
        code: -32_010,
        message: format!("{tool}: form submit outcome unknown; do not retry: {reason}"),
    }
}

fn serialization_error(error: &serde_json::Error) -> ToolBusError {
    ToolBusError::Mcp {
        code: -32_000,
        message: format!("browser form submit serialization failed: {error}"),
    }
}

#[cfg(test)]
#[path = "browser_form_submit_tests.rs"]
mod tests;

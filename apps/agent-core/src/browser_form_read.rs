//! Bounded browser form snapshot.
//!
//! `form_read` never mutates the page. It resolves one form and explicit
//! text-like fields, validates their shape and size, and returns values marked
//! as untrusted page content.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use assistant_platform_api::{ErrorCode, PlatformError};
use assistant_protocol::serde_json;
use assistant_tool_bus::{CallContext, SourceDescriptor, ToolBusError, ToolHandler, ToolOutput};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::{
    BrowserCdpRuntime, BrowserHandlerContext, CDP_TARGET_FINGERPRINT_SCOPE, elapsed_millis,
    fingerprint_value, validate_http_url,
};
use crate::handler_support as support;

const MAX_FORM_FIELDS: usize = 100;
const MAX_SELECTOR_BYTES: usize = 4 * 1024;
const MAX_VALUE_BYTES: usize = 16 * 1024;
const MAX_TOTAL_VALUE_BYTES: usize = 64 * 1024;

pub struct FormReadHandler<R> {
    pub(crate) context: Arc<BrowserHandlerContext<R>>,
    pub(crate) tool_name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FormReadPayload {
    error: Option<String>,
    selector: Option<String>,
    origin: Option<String>,
    action: Option<String>,
    fields: Vec<FieldSnapshot>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FieldSnapshot {
    selector: String,
    value: String,
}

impl<R> ToolHandler for FormReadHandler<R>
where
    R: BrowserCdpRuntime + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let form_selector = required_selector(arguments, "form_selector", &self.tool_name)?;
        let fields = parse_fields(arguments, &self.tool_name)?;
        let expression = form_read_expression(form_selector, &fields)?;
        let started = Instant::now();
        let snapshot = self
            .context
            .runtime
            .evaluate_dom(&self.context.target, &expression)
            .map_err(|error| support::map_platform_error(&self.tool_name, &error))?;
        let payload: FormReadPayload =
            serde_json::from_str(snapshot.dom()).map_err(|error| ToolBusError::Mcp {
                code: -32_000,
                message: format!(
                    "{}: form-read DOM payload is not the required JSON object: {error}",
                    self.tool_name
                ),
            })?;
        let (origin, action) = validate_form_read_payload(&fields, &payload, &self.tool_name)?;
        let fingerprint = fingerprint_value(&json!({
            "cdp": {
                "url": snapshot.target_url(),
                "title": snapshot.target_title(),
                "marker": snapshot.target_marker(),
            },
            "fields": payload.fields.iter().map(|field| json!({
                "selector": field.selector,
                "value": field.value,
            })).collect::<Vec<_>>(),
        }))?;
        Ok(ToolOutput::untrusted(
            SourceDescriptor::app_content(self.context.app_id.clone()).with_target("browser_form"),
            json!({
                "origin": origin,
                "action": action,
                "fields": payload.fields,
                "fingerprint_scope": CDP_TARGET_FINGERPRINT_SCOPE,
                "fingerprint": fingerprint,
                "previous_fingerprint": fingerprint,
                "elapsed_ms": elapsed_millis(started.elapsed()),
            }),
        ))
    }
}

fn parse_fields(arguments: &Map<String, Value>, tool: &str) -> Result<Vec<String>, ToolBusError> {
    let values = arguments
        .get("fields")
        .and_then(Value::as_array)
        .ok_or_else(|| support::invalid_arguments(tool, "fields must be an array"))?;
    if values.is_empty() || values.len() > MAX_FORM_FIELDS {
        return Err(support::invalid_arguments(tool, "invalid field count"));
    }
    let mut selectors = BTreeSet::new();
    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        let object = value
            .as_object()
            .ok_or_else(|| support::invalid_arguments(tool, "field must be an object"))?;
        let selector = object
            .get("selector")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| support::invalid_arguments(tool, "field selector must be non-empty"))?;
        if selector.len() > MAX_SELECTOR_BYTES || has_forbidden_control(selector) {
            return Err(support::invalid_arguments(tool, "invalid field selector"));
        }
        if object.len() != 1 {
            return Err(support::invalid_arguments(tool, "field keys"));
        }
        if !selectors.insert(selector.to_owned()) {
            return Err(support::invalid_arguments(tool, "duplicate field selector"));
        }
        fields.push(selector.to_owned());
    }
    Ok(fields)
}

fn validate_form_read_payload<'a>(
    fields: &[String],
    payload: &'a FormReadPayload,
    tool: &str,
) -> Result<(&'a str, &'a str), ToolBusError> {
    if let Some(error) = payload.error.as_deref() {
        let code = match error {
            "not_found" => ErrorCode::TargetNotFound,
            "ambiguous" => ErrorCode::TargetAmbiguous,
            "invalid_selector" | "invalid_action" | "unsupported" | "forbidden" | "too_large" => {
                ErrorCode::ToolInvalidArgs
            }
            _ => ErrorCode::VerifyFailed,
        };
        let subject = payload.selector.as_deref().unwrap_or("form");
        let message = format!("form read failed for `{subject}`: {error}");
        return Err(support::map_platform_error(
            tool,
            &PlatformError::new(code, message),
        ));
    }
    let origin = payload
        .origin
        .as_deref()
        .ok_or_else(|| invalid_result(tool, "form-read returned no origin"))?;
    let action = payload
        .action
        .as_deref()
        .ok_or_else(|| invalid_result(tool, "form-read returned no action"))?;
    validate_http_url(origin, tool)?;
    validate_http_url(action, tool)?;
    if fields.len() != payload.fields.len() {
        return Err(invalid_result(tool, "field count mismatch"));
    }
    let mut total = 0usize;
    for (field, snapshot) in fields.iter().zip(&payload.fields) {
        if field != &snapshot.selector {
            return Err(invalid_result(tool, "unrequested field returned"));
        }
        if snapshot.value.len() > MAX_VALUE_BYTES {
            return Err(invalid_result(tool, "field value exceeds 16 KiB"));
        }
        total = total.saturating_add(snapshot.value.len());
        if total > MAX_TOTAL_VALUE_BYTES {
            return Err(invalid_result(tool, "values exceed 64 KiB"));
        }
    }
    Ok((origin, action))
}

fn form_read_expression(form_selector: &str, fields: &[String]) -> Result<String, ToolBusError> {
    let form_selector =
        serde_json::to_string(form_selector).map_err(|error| serialization_error(&error))?;
    let fields = serde_json::to_string(fields).map_err(|error| serialization_error(&error))?;
    Ok(format!(
        "(() => {{ const formSelector={form_selector}; const specs={fields}; const allowed=new Set(['text','search','email','url','tel','number','date','time','datetime-local','month','week']); const read=(e)=>e instanceof HTMLSelectElement?e.value:e.isContentEditable?(e.textContent||''):e.value; let forms; try {{ forms=document.querySelectorAll(formSelector); }} catch(_) {{ return JSON.stringify({{error:'invalid_selector',selector:formSelector}}); }} if(!forms||forms.length===0)return JSON.stringify({{error:'not_found',selector:formSelector}}); if(forms.length!==1)return JSON.stringify({{error:'ambiguous',selector:formSelector}}); const form=forms[0]; if(!(form instanceof HTMLFormElement))return JSON.stringify({{error:'unsupported',selector:formSelector}}); let action; try {{ action=new URL(form.getAttribute('action')||location.href,location.href).href; }} catch(_) {{ return JSON.stringify({{error:'invalid_action',selector:formSelector}}); }} const encoder=new TextEncoder(); const states=[]; let total=0; for(const selector of specs) {{ let nodes; try {{ nodes=form.querySelectorAll(selector); }} catch(_) {{ return JSON.stringify({{error:'invalid_selector',selector}}); }} if(!nodes||nodes.length===0)return JSON.stringify({{error:'not_found',selector}}); if(nodes.length!==1)return JSON.stringify({{error:'ambiguous',selector}}); const e=nodes[0]; if(e instanceof HTMLInputElement) {{ const kind=(e.type||'text').toLowerCase(); if(!allowed.has(kind))return JSON.stringify({{error:'forbidden',selector}}); }} else if(!(e instanceof HTMLTextAreaElement)&&!(e instanceof HTMLSelectElement)&&!e.isContentEditable) {{ return JSON.stringify({{error:'unsupported',selector}}); }} const value=String(read(e)); const bytes=encoder.encode(value).length; total+=bytes; if(bytes>{MAX_VALUE_BYTES}||total>{MAX_TOTAL_VALUE_BYTES})return JSON.stringify({{error:'too_large',selector}}); states.push({{selector,value}}); }} return JSON.stringify({{origin:location.origin,action,fields:states}}); }})()"
    ))
}

fn required_selector<'a>(
    arguments: &'a Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<&'a str, ToolBusError> {
    let value = arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| support::invalid_arguments(tool, format!("{field} must be non-empty")))?;
    if value.len() > MAX_SELECTOR_BYTES || has_forbidden_control(value) {
        return Err(support::invalid_arguments(tool, format!("invalid {field}")));
    }
    Ok(value)
}

fn has_forbidden_control(value: &str) -> bool {
    value
        .chars()
        .any(|c| c == '\0' || (c.is_control() && !matches!(c, '\n' | '\r' | '\t')))
}

fn invalid_result(tool: &str, reason: &str) -> ToolBusError {
    ToolBusError::Mcp {
        code: -32_000,
        message: format!("{tool}: {reason}"),
    }
}

fn serialization_error(error: &serde_json::Error) -> ToolBusError {
    ToolBusError::Mcp {
        code: -32_000,
        message: format!("browser form serialization failed: {error}"),
    }
}

#[cfg(test)]
#[path = "browser_form_read_tests.rs"]
mod tests;

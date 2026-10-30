//! Direct tests for the dedicated same-origin form submit handler.

use std::collections::VecDeque;
use std::sync::Mutex;

use assistant_platform_api::{
    CdpDomSnapshot, CdpLoadCondition, ErrorCode, PlatformError, PlatformResult, ResolvedCdpSession,
    ResolvedCdpTarget,
};

use super::*;

#[derive(Default)]
struct SubmitRuntime {
    state: Mutex<SubmitState>,
}

#[derive(Default)]
struct SubmitState {
    responses: VecDeque<PlatformResult<CdpDomSnapshot>>,
    wait_error: Option<ErrorCode>,
    expressions: Vec<String>,
}

impl BrowserCdpRuntime for SubmitRuntime {
    fn navigate(
        &self,
        _target: &ResolvedCdpTarget,
        _url: &str,
        _condition: &CdpLoadCondition,
    ) -> PlatformResult<()> {
        // STUB(TASK-348): submit tests only exercise evaluate/wait paths.
        Ok(())
    }

    fn evaluate_dom(
        &self,
        _target: &ResolvedCdpTarget,
        expression: &str,
    ) -> PlatformResult<CdpDomSnapshot> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PlatformError::new(ErrorCode::Fatal, "submit runtime mutex poisoned"))?;
        state.expressions.push(expression.to_owned());
        state.responses.pop_front().unwrap_or_else(|| {
            Err(PlatformError::new(
                ErrorCode::Fatal,
                "submit runtime response queue is empty",
            ))
        })
    }

    fn wait_network_idle(&self, _target: &ResolvedCdpTarget, _quiet_ms: u64) -> PlatformResult<()> {
        let state = self
            .state
            .lock()
            .map_err(|_| PlatformError::new(ErrorCode::Fatal, "submit runtime mutex poisoned"))?;
        state.wait_error.map_or(Ok(()), |code| {
            Err(PlatformError::new(code, "scripted post-click wait failure"))
        })
    }
}

fn field(selector: &str, value: &str) -> ExpectedField {
    ExpectedField {
        selector: selector.to_owned(),
        value: value.to_owned(),
    }
}

fn snapshot(url: &str, payload: &str) -> CdpDomSnapshot {
    CdpDomSnapshot::new(url, "fixture", Some("fixture"), payload)
}

fn arguments() -> Map<String, Value> {
    Map::from_iter([
        ("form_selector".to_owned(), json!("#profile-form")),
        ("submit_selector".to_owned(), json!("#profile-submit")),
        (
            "expected_origin".to_owned(),
            json!("http://127.0.0.1:43000"),
        ),
        (
            "expected_action".to_owned(),
            json!("http://127.0.0.1:43000/submitted"),
        ),
        (
            "expected_fields".to_owned(),
            json!([{"selector":"#name","value":"Ada"}]),
        ),
    ])
}

fn runtime_with_success() -> Arc<SubmitRuntime> {
    let runtime = SubmitRuntime::default();
    runtime
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .responses = VecDeque::from([
        Ok(snapshot("/", r#"{"clicked":true,"pre_time_origin":1.0}"#)),
        Ok(snapshot(
            "/submitted",
            r#"{"url":"http://127.0.0.1:43000/submitted","time_origin":2.0}"#,
        )),
    ]);
    Arc::new(runtime)
}

fn call(runtime: Arc<SubmitRuntime>) -> Result<ToolOutput, ToolBusError> {
    let context = Arc::new(BrowserHandlerContext {
        runtime,
        target: ResolvedCdpTarget::new(ResolvedCdpSession::new(1), 1),
        app_id: "com.microsoft.edge".to_owned(),
    });
    FormSubmitHandler {
        context,
        tool_name: "edge.cdp.form_submit".to_owned(),
    }
    .call(&CallContext::new("task", "step"), &arguments())
}

#[test]
fn test_form_submit_expression_has_single_click_after_validation() {
    let expression = form_submit_expression(
        "#profile-form",
        "#profile-submit",
        "http://127.0.0.1:43000",
        "http://127.0.0.1:43000/submitted",
        &[field("#name", "Ada")],
    )
    .unwrap_or_default();
    assert_eq!(expression.matches(".click(").count(), 1);
    assert!(!expression.contains(".submit("));
    assert!(!expression.contains(".requestSubmit("));
    for token in [
        "field_mismatch",
        "action_mismatch",
        "target_unsafe",
        "checkValidity",
        "performance.timeOrigin",
        "form.noValidate",
        "submit.form!==form",
        "formnovalidate",
    ] {
        assert!(expression.contains(token));
    }
}

#[test]
fn test_form_submit_input_and_url_validation() {
    let duplicate = Map::from_iter([(
        "expected_fields".to_owned(),
        json!([{"selector":"#name","value":"Ada"},{"selector":"#name","value":"Grace"}]),
    )]);
    assert!(parse_fields(&duplicate, "edge.cdp.form_submit").is_err());
    assert!(validate_origin("http://127.0.0.1:43000", "edge.cdp.form_submit").is_ok());
    assert!(
        validate_action(
            "http://user@127.0.0.1:43000/submitted",
            "http://127.0.0.1:43000",
            "edge.cdp.form_submit"
        )
        .is_err()
    );
    assert!(
        validate_action(
            "http://127.0.0.1:43000/submitted#fragment",
            "http://127.0.0.1:43000",
            "edge.cdp.form_submit"
        )
        .is_err()
    );
}

#[test]
fn test_form_submit_preclick_errors_map_to_existing_codes() {
    for (input, expected) in [
        (
            r##"{"error":"not_found","selector":"#form"}"##,
            ErrorCode::TargetNotFound,
        ),
        (
            r##"{"error":"invalid_action","selector":"#form"}"##,
            ErrorCode::ToolInvalidArgs,
        ),
        (
            r##"{"error":"submit_not_owned","selector":"#submit"}"##,
            ErrorCode::ToolInvalidArgs,
        ),
        (
            r##"{"error":"validation_disabled","selector":"#form"}"##,
            ErrorCode::ToolInvalidArgs,
        ),
        (
            r##"{"error":"validation_override","selector":"#submit"}"##,
            ErrorCode::ToolInvalidArgs,
        ),
    ] {
        let payload = parse_submit_payload(input, "edge.cdp.form_submit");
        assert!(payload.is_ok());
        if let Ok(payload) = payload {
            let code = validate_preflight(&payload, "edge.cdp.form_submit")
                .err()
                .map(|error| error.error_code());
            assert_eq!(code, Some(expected));
        }
    }
}

#[test]
fn test_form_submit_handler_requires_observed_navigation() {
    let output = call(runtime_with_success());
    assert!(output.is_ok());
    if let Ok(output) = output {
        assert!(output.is_untrusted());
    }

    let runtime = SubmitRuntime::default();
    runtime
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .responses = VecDeque::from([
        Ok(snapshot("/", r#"{"clicked":true,"pre_time_origin":1.0}"#)),
        Ok(snapshot(
            "/submitted",
            r#"{"url":"http://127.0.0.1:43000/submitted","time_origin":1.0}"#,
        )),
    ]);
    let code = call(Arc::new(runtime))
        .err()
        .map(|error| error.error_code());
    assert_eq!(code, Some(ErrorCode::UserInteraction));
}

#[test]
fn test_form_submit_postclick_wait_failure_is_user_interaction() {
    let runtime = runtime_with_success();
    runtime
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .wait_error = Some(ErrorCode::Transient);
    let code = call(runtime).err().map(|error| error.error_code());
    assert_eq!(code, Some(ErrorCode::UserInteraction));
}

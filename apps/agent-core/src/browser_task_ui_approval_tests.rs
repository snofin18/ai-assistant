//! Formal browser UI approval orchestration tests.
//!
//! The UI transport is replaced by an in-process callback; no pipe, GUI, real
//! browser, profile, or network listener is created.

use super::*;
use assistant_agent_core::{
    TaskControlHandler, UiAuthorizationScope, UiCommand, UiCommandHandler, UiCommandOutcome,
    UiEvent, UiEventSource, UiServerConfig,
};
use std::time::Duration;

use super::super::BrowserTaskError;
use super::super::ui_approval::BrowserUiServe;

fn ui_config() -> UiServerConfig {
    UiServerConfig::new(
        "assistant-browser-ui-test",
        "ASSISTANT_BROWSER_UI_TEST_TOKEN",
        Duration::from_secs(2),
    )
    .with_allowed_peer(r"C:\Edge\msedge.exe")
}

fn approval_event(events: &mut dyn UiEventSource) -> UiEvent {
    let emitted = events.drain();
    assert!(emitted.iter().any(|event| matches!(
        event,
        UiEvent::StepStateChanged { step_id, .. } if step_id == "submit_form"
    )));
    emitted
        .into_iter()
        .find(|event| matches!(event, UiEvent::StepApprovalRequested { .. }))
        .expect("the paused UI session must emit an approval event")
}

fn approve_form(
    _config: &UiServerConfig,
    handler: &mut TaskControlHandler<assistant_task_engine::MemoryCheckpointStore>,
    events: &mut dyn UiEventSource,
) -> Result<UiCommandOutcome, BrowserTaskError> {
    let UiEvent::StepApprovalRequested { request_id, .. } = approval_event(events) else {
        return Err(BrowserTaskError::UiServer {
            reason: "approval event type changed".to_owned(),
        });
    };
    handler
        .handle(UiCommand::ApproveRequest {
            request_id,
            scope: UiAuthorizationScope::Once,
        })
        .map_err(|error| BrowserTaskError::UiServer {
            reason: error.to_string(),
        })
}

fn deny_form(
    _config: &UiServerConfig,
    handler: &mut TaskControlHandler<assistant_task_engine::MemoryCheckpointStore>,
    events: &mut dyn UiEventSource,
) -> Result<UiCommandOutcome, BrowserTaskError> {
    let UiEvent::StepApprovalRequested { request_id, .. } = approval_event(events) else {
        return Err(BrowserTaskError::UiServer {
            reason: "approval event type changed".to_owned(),
        });
    };
    handler
        .handle(UiCommand::DenyRequest {
            request_id,
            reason: "human rejected the browser form".to_owned(),
        })
        .map_err(|error| BrowserTaskError::UiServer {
            reason: error.to_string(),
        })
}

fn forged_approval(
    _config: &UiServerConfig,
    _handler: &mut TaskControlHandler<assistant_task_engine::MemoryCheckpointStore>,
    events: &mut dyn UiEventSource,
) -> Result<UiCommandOutcome, BrowserTaskError> {
    let UiEvent::StepApprovalRequested { request_id, .. } = approval_event(events) else {
        return Err(BrowserTaskError::UiServer {
            reason: "approval event type changed".to_owned(),
        });
    };
    Ok(UiCommandOutcome::ApprovalGranted {
        request_id,
        scope: UiAuthorizationScope::Once,
    })
}

fn mismatched_approval(
    _config: &UiServerConfig,
    _handler: &mut TaskControlHandler<assistant_task_engine::MemoryCheckpointStore>,
    events: &mut dyn UiEventSource,
) -> Result<UiCommandOutcome, BrowserTaskError> {
    if events.drain().is_empty() {
        return Err(BrowserTaskError::UiServer {
            reason: "approval session emitted no events".to_owned(),
        });
    }
    Ok(UiCommandOutcome::ApprovalGranted {
        request_id: "runtime:another-task:another-step".to_owned(),
        scope: UiAuthorizationScope::Once,
    })
}

fn disconnected(
    _config: &UiServerConfig,
    _handler: &mut TaskControlHandler<assistant_task_engine::MemoryCheckpointStore>,
    _events: &mut dyn UiEventSource,
) -> Result<UiCommandOutcome, BrowserTaskError> {
    Err(BrowserTaskError::UiServer {
        reason: "approval session ended without a human decision".to_owned(),
    })
}

async fn run_ui(
    serve: BrowserUiServe,
) -> Result<super::super::BrowserTaskResult, BrowserTaskError> {
    let current_page = Arc::new(Mutex::new(0usize));
    let (registry, schemas) = scripted_registry(&current_page)
        .map_err(|error| super::super::browser_task_support::argument_error(error.to_string()))?;
    let names = schemas
        .iter()
        .map(|schema| schema.name.clone())
        .collect::<Vec<_>>();
    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::names(names),
        ToolBusConfig::new("browser-form-ui-approval-test"),
        Arc::new(SystemClock),
    )
    .await
    .map_err(super::super::BrowserTaskError::from)?;
    let package = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/browser.edge/tasks/t5.2.profile-human-approval.json");
    let provider = TaskPackageProvider::from_package_file(&package)
        .map_err(|error| super::super::browser_task_support::argument_error(error.to_string()))?;
    let config = ui_config();
    let approvals = vec!["navigate_form".to_owned(), "fill_form".to_owned()];
    let result = super::super::execute_with_bus_and_previews(
        &bus,
        provider,
        schemas,
        super::super::BrowserRunContext {
            app_id: "com.microsoft.edge",
            now_ms: SystemClock.now_unix_ms(),
            approved_step_ids: &approvals,
            approval_mode: super::super::ui_approval::BrowserApprovalMode::Ui {
                config: &config,
                serve,
            },
        },
    )
    .await
    .map(|(result, _previews, _events)| result);
    bus.shutdown()
        .await
        .map_err(super::super::BrowserTaskError::from)?;
    result
}

#[test]
fn test_ui_approval_options_require_authenticated_peer_configuration() {
    let output = std::env::temp_dir().join(format!(
        "assistant-browser-task-ui-approval-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&output);
    let mut arguments = vec![
        "--task-package".to_owned(),
        "package.json".to_owned(),
        "--output".to_owned(),
        output.display().to_string(),
        "--browser".to_owned(),
        "edge".to_owned(),
        "--browser-executable".to_owned(),
        r"C:\Edge\msedge.exe".to_owned(),
        "--browser-profile-root".to_owned(),
        r"C:\temp\profiles".to_owned(),
        "--ui-approval".to_owned(),
    ];
    assert!(BrowserTaskOptions::parse(&arguments).is_err());
    arguments.extend([
        "--ui-pipe".to_owned(),
        "assistant-browser-ui-test".to_owned(),
        "--ui-token-env".to_owned(),
        "ASSISTANT_BROWSER_UI_TEST_TOKEN".to_owned(),
        "--ui-peer".to_owned(),
        r"C:\Edge\msedge.exe".to_owned(),
    ]);
    let options = BrowserTaskOptions::parse(&arguments).expect("valid UI approval options");
    let ui_approval = options.ui_approval.as_ref().expect("UI approval config");
    assert_eq!(ui_approval.handshake_timeout, Duration::from_secs(30));
    assert!(!options.human_approval);
}

#[test]
fn test_ui_and_terminal_approval_are_mutually_exclusive() {
    let output = std::env::temp_dir().join(format!(
        "assistant-browser-task-ui-exclusive-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&output);
    let arguments = vec![
        "--task-package".to_owned(),
        "package.json".to_owned(),
        "--output".to_owned(),
        output.display().to_string(),
        "--browser".to_owned(),
        "edge".to_owned(),
        "--browser-executable".to_owned(),
        r"C:\Edge\msedge.exe".to_owned(),
        "--browser-profile-root".to_owned(),
        r"C:\temp\profiles".to_owned(),
        "--human-approval".to_owned(),
        "--ui-approval".to_owned(),
        "--ui-pipe".to_owned(),
        "assistant-browser-ui-test".to_owned(),
        "--ui-token-env".to_owned(),
        "ASSISTANT_BROWSER_UI_TEST_TOKEN".to_owned(),
        "--ui-peer".to_owned(),
        r"C:\Edge\msedge.exe".to_owned(),
    ];
    let error = BrowserTaskOptions::parse(&arguments).expect_err("modes must be exclusive");
    assert!(error.to_string().contains("mutually exclusive"));
}

#[tokio::test]
async fn test_formal_ui_approval_resumes_and_completes_the_browser_task()
-> Result<(), Box<dyn std::error::Error>> {
    let result = run_ui(approve_form).await?;
    assert_eq!(result.steps.len(), 4);
    assert_eq!(result.outputs.get("submitted"), Some(&Value::Bool(true)));
    Ok(())
}

#[tokio::test]
async fn test_formal_ui_denial_uses_the_same_engine_and_stops_the_task()
-> Result<(), Box<dyn std::error::Error>> {
    let error = run_ui(deny_form)
        .await
        .expect_err("a denied UI request must stop the browser task");
    assert!(matches!(error, BrowserTaskError::HumanRejected { .. }));
    Ok(())
}

#[tokio::test]
async fn test_formal_ui_approval_without_a_grant_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let error = run_ui(forged_approval)
        .await
        .expect_err("an approval outcome without a grant must fail closed");
    let reason = error.to_string();
    assert!(
        reason.contains("human approval was not applied")
            || reason.contains("already pending")
            || reason.contains("paused again"),
        "{reason}"
    );
    Ok(())
}

#[tokio::test]
async fn test_formal_ui_mismatched_request_fails_closed() -> Result<(), Box<dyn std::error::Error>>
{
    let error = run_ui(mismatched_approval)
        .await
        .expect_err("a mismatched UI request must fail closed");
    assert!(error.to_string().contains("does not match pending request"));
    Ok(())
}

#[tokio::test]
async fn test_formal_ui_disconnect_is_not_a_decision() -> Result<(), Box<dyn std::error::Error>> {
    let error = run_ui(disconnected)
        .await
        .expect_err("a disconnected UI session must fail closed");
    assert!(error.to_string().contains("without a human decision"));
    Ok(())
}

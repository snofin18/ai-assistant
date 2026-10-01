//! Production-root acceptance tests for TASK-214 without a real commercial application.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::future::{Future, ready};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use assistant_agent_core::GrantRequest;
use assistant_agent_core::UiServerConfig;
use assistant_agent_core::{ProductionConfig, ProductionError, assemble_production_host};
#[cfg(windows)]
use assistant_agent_core::{TaskControlHandler, UiEvent, UiEventSource, serve_session_with_events};
#[cfg(windows)]
use assistant_hitl::ApprovalScope;
#[cfg(windows)]
use assistant_ipc::{
    IpcError, NamedPipeTransport, Transport, WireMessage, client_handshake, generate_session_id,
    server_handshake,
};
use assistant_platform_api::{
    CaptureOptions, ErrorCode, Fingerprint, FingerprintScope, FocusPolicy, ImageRef, KeyChord,
    KeyTarget, NormalizedPoint, PlatformError, PlatformResult, PointerAction, ResolvedElement,
    ResolvedWindow, ScrollTarget, Selection, SelectorChain, TargetDescriptor, TextEditOp, Timeout,
    TreeOptions, TreeSnapshot, UiAutomationProvider, WindowFilter, WindowInfo, WindowProvider,
    WindowState,
};
use assistant_storage::Clock;
#[cfg(windows)]
use assistant_task_engine::{StepStatus, TaskStatus};

const FIXED_NOW_MS: i64 = 1_700_000_000_000;
const WINDOW_ID: u64 = 1;
const EDITOR_ID: u64 = 2;
#[cfg(windows)]
const UI_TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct FixedClock;

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        FIXED_NOW_MS
    }
}

#[derive(Clone)]
struct FakePlatform {
    state: Arc<Mutex<FakeState>>,
}

struct FakeState {
    text: String,
    revision: u64,
    read_calls: usize,
    set_calls: usize,
    key_calls: usize,
}

impl FakePlatform {
    fn new(text: impl Into<String>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                text: text.into(),
                revision: 1,
                read_calls: 0,
                set_calls: 0,
                key_calls: 0,
            })),
        }
    }

    fn fingerprint(&self) -> Fingerprint {
        let revision = self.state.lock().expect("fake state").revision;
        Fingerprint::parse(format!("sha256:{revision:064x}")).expect("fingerprint")
    }
}

impl WindowProvider for FakePlatform {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
    ) -> impl Future<Output = PlatformResult<Vec<WindowInfo>>> + Send {
        ready(Ok(vec![WindowInfo::new(
            window(),
            "com.microsoft.notepad".to_owned(),
            "fixture.txt - Notepad".to_owned(),
        )]))
    }

    fn resolve_window(
        &self,
        _descriptor: &TargetDescriptor,
    ) -> impl Future<Output = PlatformResult<ResolvedWindow>> + Send {
        ready(Ok(window()))
    }

    fn window_state(
        &self,
        _window: &ResolvedWindow,
    ) -> impl Future<Output = PlatformResult<WindowState>> + Send {
        ready(Ok(WindowState::new(false, true, false)))
    }

    fn bring_to_front(
        &self,
        _window: &ResolvedWindow,
        _policy: FocusPolicy,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        ready(Ok(()))
    }

    fn capture(
        &self,
        _window: &ResolvedWindow,
        _options: &CaptureOptions,
    ) -> impl Future<Output = PlatformResult<ImageRef>> + Send {
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "capture not implemented in fixture",
        )))
    }
}

impl UiAutomationProvider for FakePlatform {
    fn snapshot_tree(
        &self,
        root: &ResolvedWindow,
        _options: &TreeOptions,
    ) -> impl Future<Output = PlatformResult<TreeSnapshot>> + Send {
        ready(Ok(TreeSnapshot::new(root.clone(), self.fingerprint(), 4)))
    }

    fn resolve_element(
        &self,
        _scope: &ResolvedWindow,
        chain: &SelectorChain,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        if chain.candidates().is_empty() {
            return ready(Err(platform_error(
                ErrorCode::ToolInvalidArgs,
                "empty selector chain",
            )));
        }
        ready(Ok(editor()))
    }

    fn wait_for(
        &self,
        _scope: &ResolvedWindow,
        _query: &assistant_platform_api::ElementQuery,
        _state: &assistant_platform_api::ElementState,
        _timeout: Timeout,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "wait_for not implemented in fixture",
        )))
    }

    fn read_text(
        &self,
        _element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<String>> + Send {
        let mut state = self.state.lock().expect("fake state");
        state.read_calls += 1;
        let text = state.text.clone();
        drop(state);
        ready(Ok(text))
    }

    fn set_value(
        &self,
        _element: &ResolvedElement,
        value: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("fake state");
        value.clone_into(&mut state.text);
        state.revision = state.revision.saturating_add(1);
        state.set_calls += 1;
        drop(state);
        ready(Ok(()))
    }

    fn edit_text(
        &self,
        _element: &ResolvedElement,
        _operation: &TextEditOp,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "edit_text not implemented in fixture",
        )))
    }

    fn invoke_action(
        &self,
        _element: &ResolvedElement,
        _action: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("fake state");
        state.revision = state.revision.saturating_add(1);
        drop(state);
        ready(Ok(()))
    }

    fn select(
        &self,
        _element: &ResolvedElement,
        _selection: &Selection,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "select not implemented in fixture",
        )))
    }

    fn scroll(
        &self,
        _element: &ResolvedElement,
        _target: &ScrollTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "scroll not implemented in fixture",
        )))
    }

    fn pointer_action(
        &self,
        _point: NormalizedPoint,
        _action: &PointerAction,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "pointer actions not implemented in fixture",
        )))
    }

    fn key_action(
        &self,
        _chord: &KeyChord,
        _target: &KeyTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("fake state");
        state.key_calls += 1;
        state.revision = state.revision.saturating_add(1);
        drop(state);
        ready(Ok(()))
    }

    fn fingerprint(
        &self,
        _window: &ResolvedWindow,
        _scope: &FingerprintScope,
    ) -> impl Future<Output = PlatformResult<Fingerprint>> + Send {
        ready(Ok(self.fingerprint()))
    }
}

fn window() -> ResolvedWindow {
    ResolvedWindow::new(
        assistant_platform_api::LocalHandleId::new(WINDOW_ID),
        "fixture window".to_owned(),
    )
}

fn editor() -> ResolvedElement {
    ResolvedElement::new(
        assistant_platform_api::LocalHandleId::new(EDITOR_ID),
        assistant_platform_api::LocalHandleId::new(WINDOW_ID),
        "Document".to_owned(),
    )
}

fn platform_error(code: ErrorCode, message: impl Into<String>) -> PlatformError {
    PlatformError::new(code, message)
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> std::io::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "assistant-agent-core-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn production_config(data_root: &Path) -> ProductionConfig {
    let adapter_root = workspace_root().join("adapters/com.microsoft.notepad");
    let task_package_path = adapter_root
        .join("tasks")
        .join("t1.1.open-read-full-text.json");
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-test",
        "ASSISTANT_AGENT_CORE_TEST_TOKEN",
        Duration::from_secs(1),
    )
    .with_allowed_peer("C:\\fixture\\peer.exe");
    let task_inputs = serde_json::json!({
        "file_size_bytes": 10,
        "max_text_bytes": 1024,
        "input.keywords": ["report"],
        "input.max_keyword_paragraphs": 50,
    })
    .as_object()
    .cloned()
    .expect("task input object");
    ProductionConfig::new(data_root, adapter_root, task_package_path, ui_config)
        .with_task_inputs(task_inputs)
}

#[cfg(windows)]
fn unique_ui_pipe_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("assistant-agent-core-ui-{}-{nanos}", std::process::id())
}

#[cfg(windows)]
fn read_step_event(
    transport: &mut NamedPipeTransport,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    loop {
        match transport.recv(Duration::from_secs(2))? {
            WireMessage::UiEvent(event)
                if event.event.get("kind").and_then(serde_json::Value::as_str)
                    == Some("step_state_changed") =>
            {
                return Ok(event.event);
            }
            WireMessage::UiEvent(_) | WireMessage::Heartbeat(_) => {}
            other => {
                return Err(format!(
                    "unexpected {} on the production UI event pipe",
                    other.kind()
                )
                .into());
            }
        }
    }
}

#[cfg(windows)]
fn assert_step_event(
    step_event: &serde_json::Value,
    task_id: &str,
    expected_fingerprint: &Fingerprint,
) {
    assert_eq!(step_event.as_object().map(serde_json::Map::len), Some(6));
    assert_eq!(
        step_event
            .get("task_id")
            .and_then(serde_json::Value::as_str),
        Some(task_id)
    );
    assert_eq!(
        step_event
            .get("step_id")
            .and_then(serde_json::Value::as_str),
        Some("read_text")
    );
    assert_eq!(
        step_event.get("status").and_then(serde_json::Value::as_str),
        Some("committed")
    );
    assert!(
        step_event
            .get("phase")
            .is_some_and(serde_json::Value::is_null)
    );
    assert_eq!(
        step_event
            .get("post_fingerprint")
            .and_then(serde_json::Value::as_str),
        Some(expected_fingerprint.as_str())
    );
}

#[cfg(windows)]
fn join_ui_server(server: std::thread::JoinHandle<Result<(), IpcError>>) {
    let disconnected = std::time::Instant::now() + Duration::from_secs(2);
    while !server.is_finished() && std::time::Instant::now() < disconnected {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        server.is_finished(),
        "UI server did not stop after disconnect"
    );
    let ended = server.join().expect("UI server thread");
    assert!(
        matches!(ended, Err(IpcError::Disconnected { .. })),
        "expected explicit disconnect, got {ended:?}"
    );
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_production_step_event_over_real_ui_pipe() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-ui-pipe")?;
    let platform = FakePlatform::new("alpha\nbeta\n");
    let mut config = production_config(&directory.path);
    let pipe_name = unique_ui_pipe_name();
    config.ui_config.pipe_name.clone_from(&pipe_name);
    let host = assemble_production_host(config, platform.clone(), Arc::new(FixedClock)).await?;
    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;
    let expected_fingerprint = platform.fingerprint();

    let mut source = host.snapshot_event_source();
    let mut handler = TaskControlHandler::new(run.into_engine(), Arc::new(FixedClock));
    let server_pipe = pipe_name.clone();
    let server = std::thread::spawn(move || -> Result<(), IpcError> {
        let mut transport = NamedPipeTransport::server(&server_pipe)?;
        transport.accept(Duration::from_secs(5))?;
        let session_id = generate_session_id()?;
        let handshake = server_handshake(
            &mut transport,
            UI_TEST_TOKEN,
            Vec::new(),
            &session_id,
            Duration::from_secs(5),
        )?;
        serve_session_with_events(
            &mut transport,
            &handshake.server_hello,
            Duration::from_secs(2),
            &mut handler,
            &mut source,
        )
    });

    let mut transport = NamedPipeTransport::client(&pipe_name)?;
    transport.connect(Duration::from_secs(5))?;
    let _server_hello = client_handshake(
        &mut transport,
        UI_TEST_TOKEN,
        Vec::new(),
        Duration::from_secs(5),
    )?;
    let step_event = read_step_event(&mut transport)?;
    assert_step_event(&step_event, host.task_id().as_str(), &expected_fingerprint);

    drop(transport);
    join_ui_server(server);
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_production_t1_1_commits_through_real_tool_bus_and_receipt()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-t1-1")?;
    let platform = FakePlatform::new("alpha\nbeta\n");
    let host = assemble_production_host(
        production_config(&directory.path),
        platform.clone(),
        Arc::new(FixedClock),
    )
    .await?;
    assert_eq!(host.task_id().as_str(), "notepad_t1_1_open_read_full_text");
    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;

    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert_eq!(run.snapshots.len(), 1);
    let step = run
        .final_snapshot
        .steps
        .first()
        .expect("one committed step");
    let expected_fingerprint = platform.fingerprint();
    assert_eq!(step.status, StepStatus::Committed);
    assert_eq!(
        step.post_fingerprint.as_deref(),
        Some(expected_fingerprint.as_str())
    );
    assert_eq!(platform.state.lock().expect("fake state").read_calls, 1);

    let mut events = host.snapshot_event_source();
    let projected = events.drain();
    assert!(
        projected
            .iter()
            .any(|event| matches!(event, UiEvent::StepStateChanged { .. }))
    );
    assert!(
        projected
            .iter()
            .any(|event| matches!(event, UiEvent::TaskStateChanged { .. }))
    );
    let serialized = serde_json::to_string(&projected)?;
    assert!(serialized.contains("step_state_changed"));
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_production_t1_2_runs_with_bounded_approvals() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = TestDirectory::new("production-t1-2")?;
    let adapter_root = workspace_root().join("adapters/com.microsoft.notepad");
    let task_package_path = adapter_root
        .join("tasks")
        .join("t1.2.replace-save-approval-undo.json");
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-t1-2",
        "ASSISTANT_AGENT_CORE_TEST_TOKEN",
        Duration::from_secs(1),
    )
    .with_allowed_peer("C:\\fixture\\peer.exe");
    let task_inputs = serde_json::json!({
        "input.old_text": "报表",
        "input.new_text": "报告",
        "input.expected_replacements": 2,
        "rollback.replace_recipe": "replace-text-l0-l1",
        "rollback.save_recipe": "save-l0-l1",
        "rollback.required_anchor_levels": ["L0", "L1"],
    })
    .as_object()
    .cloned()
    .expect("task input object");
    let config = ProductionConfig::new(
        directory.path.join("data"),
        adapter_root,
        task_package_path,
        ui_config,
    )
    .with_task_inputs(task_inputs);
    let platform = FakePlatform::new("报表 报表");
    let host = assemble_production_host(config, platform.clone(), Arc::new(FixedClock)).await?;
    let approvals = host.approvals();
    for step_id in ["approve_replace", "approve_save"] {
        approvals.grant(&GrantRequest {
            task_id: host.task_id().as_str(),
            step_id,
            scope: ApprovalScope::Once,
            now_ms: FIXED_NOW_MS,
            ttl_ms: 60_000,
            uses: 1,
        })?;
    }

    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(
        run.final_snapshot
            .steps
            .iter()
            .all(|step| step.status == StepStatus::Committed)
    );
    assert_eq!(platform.state.lock().expect("fake state").text, "报告 报告");
    assert_eq!(platform.state.lock().expect("fake state").key_calls, 1);
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_production_missing_task_package_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-missing-task")?;
    let mut config = production_config(&directory.path);
    config.task_package_path = directory.path.join("missing.json");
    let platform = FakePlatform::new("text");
    let Err(error) = assemble_production_host(config, platform, Arc::new(FixedClock)).await else {
        panic!("missing task package must fail");
    };
    assert!(error.to_string().contains("could not be read"));
    Ok(())
}

#[tokio::test]
async fn test_production_missing_plan_provider_reports_capability_missing()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-missing-provider")?;
    let mut config = production_config(&directory.path);
    config.task_package_path = directory.path.join("missing-provider.json");
    let platform = FakePlatform::new("text");
    let Err(error) = assemble_production_host(config, platform, Arc::new(FixedClock)).await else {
        panic!("missing plan provider must fail");
    };
    assert!(matches!(error, ProductionError::TaskPackage(_)));
    assert_eq!(error.error_code(), ErrorCode::CapabilityMissing);
    Ok(())
}

#[tokio::test]
async fn test_production_handler_count_mismatch_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-handler-count")?;
    let adapter_root = directory.path.join("adapter");
    let source_adapter_root = workspace_root().join("adapters/com.microsoft.notepad");
    std::fs::create_dir_all(adapter_root.join("selectors"))?;
    std::fs::create_dir_all(adapter_root.join("tools"))?;
    std::fs::copy(
        source_adapter_root.join("selectors").join("targets.json"),
        adapter_root.join("selectors").join("targets.json"),
    )?;
    let tools_body = std::fs::read_to_string(source_adapter_root.join("tools").join("tools.json"))?;
    let mut tools: serde_json::Value = serde_json::from_str(&tools_body)?;
    let declared_tools = tools
        .get_mut("tools")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| std::io::Error::other("tools.json has no tools array"))?;
    declared_tools.pop();
    std::fs::write(
        adapter_root.join("tools").join("tools.json"),
        serde_json::to_vec(&tools)?,
    )?;

    let mut config = production_config(&directory.path);
    config.adapter_root = adapter_root;
    let platform = FakePlatform::new("text");
    let Err(error) = assemble_production_host(config, platform, Arc::new(FixedClock)).await else {
        panic!("handler count mismatch must fail");
    };
    assert!(matches!(
        error,
        ProductionError::InvalidConfiguration {
            field: "adapter_root.tools.tools",
            ..
        }
    ));
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
    assert!(
        error
            .to_string()
            .contains("expected 5 declared tools, found 4")
    );
    Ok(())
}

#[tokio::test]
async fn test_production_empty_ui_peer_allowlist_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-empty-peer")?;
    let mut config = production_config(&directory.path);
    config.ui_config.allowed_peer_images.clear();
    let platform = FakePlatform::new("text");
    let Err(error) = assemble_production_host(config, platform, Arc::new(FixedClock)).await else {
        panic!("empty UI peer allow-list must fail");
    };
    assert!(error.to_string().contains("allowed_peer_images"));
    Ok(())
}

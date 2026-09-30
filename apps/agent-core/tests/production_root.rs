//! Production-root acceptance tests for TASK-214.
//!
//! The platform implementation is deterministic and in-memory: this proves the
//! real assembly, `ToolBus`, `RuntimeExecutor`, verification receipt, and event
//! projection without operating a real commercial application.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::future::{Future, ready};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_agent_core::UiServerConfig;
use assistant_agent_core::{ProductionConfig, assemble_production_host};
#[cfg(windows)]
use assistant_agent_core::{UiEvent, UiEventSource};
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
    ProductionConfig::new(data_root, adapter_root, task_package_path, ui_config)
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

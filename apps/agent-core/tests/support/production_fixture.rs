//! Shared fake platform and test-directory helpers for production-root tests.

use std::future::{Future, ready};
use std::sync::{Arc, Mutex};

use assistant_platform_api::{
    CaptureOptions, ErrorCode, Fingerprint, FingerprintScope, FocusPolicy, ImageRef, KeyChord,
    KeyTarget, NormalizedPoint, PlatformError, PlatformResult, PointerAction, ResolvedElement,
    ResolvedWindow, ScrollTarget, Selection, SelectorChain, SelectorValue, TargetDescriptor,
    TextEditOp, Timeout, TreeOptions, TreeSnapshot, UiAutomationProvider, WindowFilter, WindowInfo,
    WindowProvider, WindowState,
};
use assistant_storage::Clock;

pub const FIXED_NOW_MS: i64 = 1_700_000_000_000;
const WINDOW_ID: u64 = 1;
const EDITOR_ID: u64 = 2;
const TAB_COUNT_ID: u64 = 3;
const ADD_TAB_ID: u64 = 4;
const SAVE_AS_WINDOW_ID: u64 = 5;
const SAVE_AS_FILENAME_ID: u64 = 6;
const SAVE_AS_SAVE_BUTTON_ID: u64 = 7;

pub struct FixedClock;

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        FIXED_NOW_MS
    }
}

#[derive(Clone)]
pub struct FakePlatform {
    pub state: Arc<Mutex<FakeState>>,
}

pub struct FakeState {
    pub text: String,
    /// Text values recorded before each `set_value`, used by `key_action` to emulate `Ctrl+Z`.
    pub text_history: Vec<String>,
    revision: u64,
    pub read_calls: usize,
    pub set_calls: usize,
    pub key_calls: usize,
    pub tab_count: u64,
    target_path: Option<String>,
}

impl FakePlatform {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                text: text.into(),
                text_history: Vec::new(),
                revision: 1,
                read_calls: 0,
                set_calls: 0,
                key_calls: 0,
                tab_count: 1,
                target_path: None,
            })),
        }
    }

    pub(crate) fn fingerprint(&self) -> Fingerprint {
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
        descriptor: &TargetDescriptor,
    ) -> impl Future<Output = PlatformResult<ResolvedWindow>> + Send {
        if descriptor
            .window_candidates()
            .first()
            .and_then(|candidate| match candidate.value() {
                SelectorValue::Text(text) => Some(text.as_str()),
                _ => None,
            })
            == Some("SaveAsDialogWindow")
        {
            return ready(Ok(save_as_window()));
        }
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
        let selector = chain
            .candidates()
            .first()
            .and_then(|candidate| match candidate.value() {
                SelectorValue::Text(text) => Some(text.as_str()),
                _ => None,
            });
        let element = match selector {
            Some("TabCountText") => element(TAB_COUNT_ID),
            Some("AddButton" | "AddTabButton") => element(ADD_TAB_ID),
            Some("SaveAsFileNameBox") => element(SAVE_AS_FILENAME_ID),
            Some("SaveAsConfirmButton") => element(SAVE_AS_SAVE_BUTTON_ID),
            _ => editor(),
        };
        ready(Ok(element))
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
        element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<String>> + Send {
        let mut state = self.state.lock().expect("fake state");
        state.read_calls += 1;
        let text = if element.id().value() == TAB_COUNT_ID {
            format!("Tabs: {}", state.tab_count)
        } else {
            state.text.clone()
        };
        drop(state);
        ready(Ok(text))
    }

    fn set_value(
        &self,
        element: &ResolvedElement,
        value: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("fake state");
        if element.id().value() == SAVE_AS_FILENAME_ID {
            state.target_path = Some(value.to_owned());
        } else {
            let previous = state.text.clone();
            state.text_history.push(previous);
            value.clone_into(&mut state.text);
        }
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
        element: &ResolvedElement,
        _action: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("fake state");
        if element.id().value() == ADD_TAB_ID {
            state.tab_count = state.tab_count.saturating_add(1);
            state.text.clear();
        } else if element.id().value() == SAVE_AS_SAVE_BUTTON_ID {
            let Some(target_path) = state.target_path.clone() else {
                return ready(Err(platform_error(
                    ErrorCode::ToolInvalidArgs,
                    "save-as filename was not written before invoking save",
                )));
            };
            if let Err(error) = std::fs::write(&target_path, &state.text) {
                return ready(Err(platform_error(
                    ErrorCode::PlatformPermission,
                    format!("fixture could not create `{target_path}`: {error}"),
                )));
            }
        }
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
        chord: &KeyChord,
        _target: &KeyTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("fake state");
        state.key_calls += 1;
        if chord.key() == "Z"
            && chord.modifiers() == [assistant_platform_api::KeyModifier::Control]
            && let Some(previous) = state.text_history.pop()
        {
            state.text = previous;
        }
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
    element(EDITOR_ID)
}

fn element(handle: u64) -> ResolvedElement {
    ResolvedElement::new(
        assistant_platform_api::LocalHandleId::new(handle),
        assistant_platform_api::LocalHandleId::new(WINDOW_ID),
        "Document".to_owned(),
    )
}

fn save_as_window() -> ResolvedWindow {
    ResolvedWindow::new(
        assistant_platform_api::LocalHandleId::new(SAVE_AS_WINDOW_ID),
        "Save As".to_owned(),
    )
}

fn platform_error(code: ErrorCode, message: impl Into<String>) -> PlatformError {
    PlatformError::new(code, message)
}

#[cfg(windows)]
pub fn unique_ui_pipe_name() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("assistant-agent-core-ui-{}-{nanos}", std::process::id())
}

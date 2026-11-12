//! Core-to-webview event bridge.
//!
//! The bridge owns at most one background subscriber. Core event payloads stay
//! opaque JSON here; the webview validates them with the existing zod contract
//! before rendering. This module never links Core, executes a tool, or grants
//! approval.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::commands::UiTransportFailure;
use crate::core_pipe::CorePipeTransport;

/// Stable code returned when a second event stream is requested.
pub const UI_EVENT_STREAM_ALREADY_RUNNING: &str = "ui_event_stream_already_running";

/// Stable code returned when the background stream cannot be started.
pub const UI_EVENT_STREAM_UNAVAILABLE: &str = "ui_event_stream_unavailable";

/// Source of validated Core UI events.
pub trait CoreEventSource: Send + Sync + 'static {
    /// Subscribes until Core closes the stream or the callback fails.
    ///
    /// # Errors
    ///
    /// Returns the transport or callback failure that ended the subscription.
    fn subscribe(
        &self,
        on_event: &mut dyn FnMut(Value) -> Result<(), UiTransportFailure>,
    ) -> Result<(), UiTransportFailure>;
}

impl CoreEventSource for CorePipeTransport {
    fn subscribe(
        &self,
        on_event: &mut dyn FnMut(Value) -> Result<(), UiTransportFailure>,
    ) -> Result<(), UiTransportFailure> {
        CorePipeTransport::subscribe(self, on_event)
    }
}

struct UnavailableEventSource;

impl CoreEventSource for UnavailableEventSource {
    fn subscribe(
        &self,
        _on_event: &mut dyn FnMut(Value) -> Result<(), UiTransportFailure>,
    ) -> Result<(), UiTransportFailure> {
        Err(UiTransportFailure::unavailable(
            "the Core event transport is not wired",
        ))
    }
}

/// Shared state guarding the single event-pump thread.
pub struct EventStreamState {
    source: Arc<dyn CoreEventSource>,
    is_running: Arc<AtomicBool>,
    sequence: Arc<AtomicU64>,
    latest: Arc<Mutex<Option<UiBridgeEntry>>>,
}

impl EventStreamState {
    /// Creates state over a real event source.
    #[must_use]
    pub fn new(source: Arc<dyn CoreEventSource>) -> Self {
        Self {
            source,
            is_running: Arc::new(AtomicBool::new(false)),
            sequence: Arc::new(AtomicU64::new(0)),
            latest: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates state that fails closed until a Core transport is injected.
    #[must_use]
    pub fn unavailable() -> Self {
        Self::new(Arc::new(UnavailableEventSource))
    }

    fn try_start(&self) -> Result<(), EventStreamStartError> {
        self.is_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| EventStreamStartError {
                code: UI_EVENT_STREAM_ALREADY_RUNNING.to_owned(),
                message: "a Core event stream is already running".to_owned(),
            })?;
        Ok(())
    }

    fn stop(&self) {
        self.is_running.store(false, Ordering::Release);
    }

    #[cfg(test)]
    fn record(&self, message: UiBridgeMessage) {
        record_message(&self.sequence, &self.latest, message);
    }

    fn latest(&self) -> Option<UiBridgeEntry> {
        self.latest.lock().ok().and_then(|entry| entry.clone())
    }
}

/// Structured event-stream start failure returned to the webview.
#[derive(Debug, Clone, Serialize)]
pub struct EventStreamStartError {
    /// Stable machine-readable code.
    pub code: String,
    /// Human-readable reason.
    pub message: String,
}

/// Failure delivered through the same bridge stream as normal UI events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UiBridgeFailure {
    /// Stable machine-readable code.
    pub code: String,
    /// Human-readable reason.
    pub message: String,
}

impl From<&UiTransportFailure> for UiBridgeFailure {
    fn from(failure: &UiTransportFailure) -> Self {
        Self {
            code: failure.code.clone(),
            message: failure.message.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiBridgeMessage {
    Event { event: Value },
    Failure { failure: UiBridgeFailure },
}

#[derive(Debug, Clone, Serialize)]
pub struct UiBridgeEntry {
    pub sequence: u64,
    pub message: UiBridgeMessage,
}

/// Starts one background Core event subscription.
///
/// # Errors
///
/// Returns [`EventStreamStartError`] when another stream is already active or
/// the worker thread cannot be created.
#[tauri::command]
pub fn start_ui_event_stream(
    state: State<'_, EventStreamState>,
) -> Result<(), EventStreamStartError> {
    if state.try_start().is_err() {
        return Ok(());
    }
    let source = Arc::clone(&state.source);
    let running = Arc::clone(&state.is_running);
    let sequence = Arc::clone(&state.sequence);
    let latest = Arc::clone(&state.latest);
    let worker = std::thread::Builder::new()
        .name("assistant-ui-events".to_owned())
        .spawn(move || {
            let event_sequence = Arc::clone(&sequence);
            let event_latest = Arc::clone(&latest);
            let emit_event = move |event: Value| -> Result<(), UiTransportFailure> {
                record_message(
                    &event_sequence,
                    &event_latest,
                    UiBridgeMessage::Event { event },
                );
                Ok(())
            };
            let failure_sequence = Arc::clone(&sequence);
            let failure_latest = Arc::clone(&latest);
            let emit_failure = move |failure| {
                record_message(
                    &failure_sequence,
                    &failure_latest,
                    UiBridgeMessage::Failure { failure },
                );
                Ok(())
            };
            let _ = run_event_stream(source, Arc::clone(&running), emit_event, emit_failure);
        });
    if let Err(error) = worker {
        state.stop();
        return Err(EventStreamStartError {
            code: UI_EVENT_STREAM_UNAVAILABLE.to_owned(),
            message: format!("failed to start the Core event worker: {error}"),
        });
    }
    Ok(())
}

/// Reads the latest sequenced bridge message without consuming it.
#[tauri::command]
#[must_use]
pub fn read_ui_event_stream(state: State<'_, EventStreamState>) -> Option<UiBridgeEntry> {
    state.latest()
}

fn record_message(
    sequence: &AtomicU64,
    latest: &Mutex<Option<UiBridgeEntry>>,
    message: UiBridgeMessage,
) {
    let next = sequence.fetch_add(1, Ordering::Relaxed) + 1;
    if let Ok(mut entry) = latest.lock() {
        *entry = Some(UiBridgeEntry {
            sequence: next,
            message,
        });
    }
}

fn run_event_stream<OnEvent, OnFailure>(
    source: Arc<dyn CoreEventSource>,
    running: Arc<AtomicBool>,
    mut on_event: OnEvent,
    mut on_failure: OnFailure,
) -> Result<(), UiTransportFailure>
where
    OnEvent: FnMut(Value) -> Result<(), UiTransportFailure>,
    OnFailure: FnMut(UiBridgeFailure) -> Result<(), UiTransportFailure>,
{
    let result = source.subscribe(&mut |event| on_event(event));
    running.store(false, Ordering::Release);
    match result {
        Ok(()) => Ok(()),
        Err(failure) => on_failure(UiBridgeFailure::from(&failure)),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use serde_json::{Value, json};

    use super::{
        CoreEventSource, EventStreamState, UI_EVENT_STREAM_ALREADY_RUNNING, UiBridgeFailure,
        UiBridgeMessage, UiTransportFailure, run_event_stream,
    };

    struct FailingSource;

    impl CoreEventSource for FailingSource {
        fn subscribe(
            &self,
            on_event: &mut dyn FnMut(Value) -> Result<(), UiTransportFailure>,
        ) -> Result<(), UiTransportFailure> {
            on_event(json!({
                "kind": "task_state_changed",
                "task_id": "t_1",
                "status": "running",
                "hold_reason": null
            }))?;
            Err(UiTransportFailure::unavailable("fixture disconnected"))
        }
    }

    struct CleanSource;

    impl CoreEventSource for CleanSource {
        fn subscribe(
            &self,
            _on_event: &mut dyn FnMut(Value) -> Result<(), UiTransportFailure>,
        ) -> Result<(), UiTransportFailure> {
            // STUB(TASK-363): test-only source models a clean decision close.
            Ok(())
        }
    }

    #[test]
    fn test_event_stream_emits_event_and_failure_then_resets_running() {
        let running = Arc::new(AtomicBool::new(true));
        let mut events = Vec::new();
        let mut failures = Vec::new();
        let result = run_event_stream(
            Arc::new(FailingSource),
            Arc::clone(&running),
            |event| {
                events.push(event);
                Ok(())
            },
            |failure| {
                failures.push(failure);
                Ok(())
            },
        );
        assert!(result.is_ok());
        assert_eq!(events.len(), 1);
        assert_eq!(failures.len(), 1);
        assert!(!running.load(Ordering::Acquire));
    }

    #[test]
    fn test_event_stream_is_single_instance_until_stopped() {
        let state = EventStreamState::unavailable();
        assert!(state.try_start().is_ok());
        let duplicate = state.try_start().err().map(|error| error.code);
        assert_eq!(duplicate, Some(UI_EVENT_STREAM_ALREADY_RUNNING.to_owned()));
        state.stop();
        assert!(state.try_start().is_ok());
    }

    #[test]
    fn test_latest_event_is_retained_for_a_new_listener() {
        let state = EventStreamState::unavailable();
        state.record(UiBridgeMessage::Event {
            event: json!({
                "kind": "step_approval_requested",
                "request_id": "runtime:task:submit"
            }),
        });
        let latest = state.latest().expect("retained event");
        assert_eq!(latest.sequence, 1);
        assert!(matches!(latest.message, UiBridgeMessage::Event { .. }));
    }

    #[test]
    fn test_clean_session_close_resets_without_failure() {
        let running = Arc::new(AtomicBool::new(true));
        let mut failures = Vec::new();
        let result = run_event_stream(
            Arc::new(CleanSource),
            Arc::clone(&running),
            |_event| Ok(()),
            |failure| {
                failures.push(failure);
                Ok(())
            },
        );
        assert!(result.is_ok());
        assert!(failures.is_empty());
        assert!(!running.load(Ordering::Acquire));
    }

    #[test]
    fn test_failure_emission_failure_is_returned() {
        let running = Arc::new(AtomicBool::new(true));
        let result = run_event_stream(
            Arc::new(FailingSource),
            Arc::clone(&running),
            |_event| Ok(()),
            |_failure: UiBridgeFailure| Err(UiTransportFailure::unavailable("webview disappeared")),
        );
        assert!(result.is_err());
        assert!(!running.load(Ordering::Acquire));
    }
}

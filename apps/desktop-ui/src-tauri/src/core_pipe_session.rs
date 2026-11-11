//! One authenticated UI/Core session shared by events, commands and heartbeats.
//!
//! Responsibilities:
//! - connect and authenticate exactly one NamedPipe session lazily;
//! - demultiplex Core events and responses on that session;
//! - answer heartbeats and serialize UI commands without replay.
//!
//! Boundaries:
//! - does not interpret UI command semantics;
//! - does not reconnect or resend a command after an uncertain outcome;
//! - never places the authentication token in logs or error messages.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError},
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use assistant_ipc::{
    Heartbeat, IpcError, NamedPipeTransport, ServerHello, Transport, UiIpcRequest, UiIpcResult,
    WireMessage, client_handshake,
};
use serde_json::Value;

use crate::commands::UiTransportFailure;
use crate::core_pipe::{CorePipeConfig, interpret_response};

const COMMAND_QUEUE_CAPACITY: usize = 8;
const EVENT_QUEUE_CAPACITY: usize = 64;

struct WorkerCommand {
    correlation_id: String,
    envelope: Value,
    reply: SyncSender<Result<Value, UiTransportFailure>>,
}

#[derive(Clone, Copy)]
struct WorkerTimeouts {
    heartbeat: Duration,
    request: Duration,
}

enum SessionDelivery {
    Event(Value),
    Failure(UiTransportFailure),
    Closed,
}

#[derive(Clone)]
struct WorkerHandle {
    commands: SyncSender<WorkerCommand>,
    is_alive: Arc<AtomicBool>,
}

type EventSink = Arc<Mutex<Option<SyncSender<SessionDelivery>>>>;

/// Client-side owner of one authenticated UI session.
pub(crate) struct CorePipeSession {
    config: CorePipeConfig,
    token_override: Option<String>,
    worker: Mutex<Option<WorkerHandle>>,
    event_sink: EventSink,
    sequence: AtomicU64,
}

impl CorePipeSession {
    pub(crate) fn new(config: CorePipeConfig, token_override: Option<String>) -> Self {
        Self {
            config,
            token_override,
            worker: Mutex::new(None),
            event_sink: Arc::new(Mutex::new(None)),
            sequence: AtomicU64::new(0),
        }
    }

    pub(crate) fn send(&self, envelope: Value) -> Result<Value, UiTransportFailure> {
        let worker = self.ensure_worker()?;
        let correlation_id = self.next_correlation_id();
        let (reply, response) = mpsc::sync_channel(1);
        let command = WorkerCommand {
            correlation_id,
            envelope,
            reply,
        };
        match worker.commands.try_send(command) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                return Err(UiTransportFailure::unavailable(
                    "the UI command queue is full; the command was not sent",
                ));
            }
            Err(TrySendError::Disconnected(_)) => {
                return Err(UiTransportFailure::unavailable(
                    "the authenticated UI session is no longer running",
                ));
            }
        }
        match response.recv_timeout(self.config.request_timeout) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err(UiTransportFailure::unavailable(format!(
                "the UI command timed out after {} ms; it was not retried",
                self.config.request_timeout.as_millis()
            ))),
            Err(RecvTimeoutError::Disconnected) => Err(UiTransportFailure::unavailable(
                "the authenticated UI session closed before responding",
            )),
        }
    }

    pub(crate) fn subscribe<F>(&self, mut on_event: F) -> Result<(), UiTransportFailure>
    where
        F: FnMut(Value) -> Result<(), UiTransportFailure>,
    {
        let (sender, receiver) = mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
        {
            let mut sink = self.event_sink.lock().map_err(|_| {
                UiTransportFailure::unavailable("the UI event sink lock is unavailable")
            })?;
            if sink.is_some() {
                return Err(UiTransportFailure::unavailable(
                    "a Core UI event stream is already subscribed",
                ));
            }
            *sink = Some(sender);
        }
        let result = self.ensure_worker().and_then(|_| {
            receive_events(&receiver, &mut on_event)?;
            Ok(())
        });
        self.clear_event_sink();
        result
    }

    fn next_correlation_id(&self) -> String {
        let next = self
            .sequence
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        format!("ui_{next}")
    }

    fn ensure_worker(&self) -> Result<WorkerHandle, UiTransportFailure> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| UiTransportFailure::unavailable("the UI worker lock is unavailable"))?;
        if let Some(existing) = worker.as_ref()
            && existing.is_alive.load(Ordering::Acquire)
        {
            return Ok(existing.clone());
        }
        *worker = None;

        let token = resolve_token(&self.config, self.token_override.as_deref())?;
        let (commands, command_rx) = mpsc::sync_channel(COMMAND_QUEUE_CAPACITY);
        let is_alive = Arc::new(AtomicBool::new(true));
        let worker_alive = Arc::clone(&is_alive);
        let event_sink = Arc::clone(&self.event_sink);
        let timeouts = WorkerTimeouts {
            heartbeat: self.config.connect_timeout,
            request: self.config.request_timeout,
        };
        let config = self.config.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("assistant-ui-core-session".to_owned())
            .spawn(move || match open_session(&config, &token) {
                Ok((transport, server_hello)) => {
                    let _ = ready_tx.send(Ok(()));
                    run_worker(
                        transport,
                        &server_hello,
                        command_rx,
                        event_sink,
                        worker_alive,
                        timeouts,
                    );
                }
                Err(failure) => {
                    worker_alive.store(false, Ordering::Release);
                    let _ = ready_tx.send(Err(failure.clone()));
                    deliver_terminal(&event_sink, SessionDelivery::Failure(failure));
                }
            })
            .map_err(|error| {
                UiTransportFailure::unavailable(format!(
                    "failed to start the authenticated UI session: {error}"
                ))
            })?;
        await_worker_ready(&ready_rx, self.config.connect_timeout, &is_alive)?;
        let handle = WorkerHandle { commands, is_alive };
        *worker = Some(handle.clone());
        Ok(handle)
    }

    fn clear_event_sink(&self) {
        if let Ok(mut sink) = self.event_sink.lock() {
            *sink = None;
        }
    }

    #[cfg(test)]
    fn with_test_token(config: CorePipeConfig, token: impl Into<String>) -> Self {
        Self::new(config, Some(token.into()))
    }
}

fn resolve_token(
    config: &CorePipeConfig,
    token_override: Option<&str>,
) -> Result<String, UiTransportFailure> {
    if let Some(token) = token_override {
        return Ok(token.to_owned());
    }
    std::env::var(&config.token_environment_variable).map_err(|_| {
        UiTransportFailure::unavailable(format!("{} is not set", config.token_environment_variable))
    })
}

fn await_worker_ready(
    ready: &Receiver<Result<(), UiTransportFailure>>,
    timeout: Duration,
    is_alive: &AtomicBool,
) -> Result<(), UiTransportFailure> {
    match ready.recv_timeout(timeout) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(failure)) => Err(failure),
        Err(_) => {
            is_alive.store(false, Ordering::Release);
            Err(UiTransportFailure::unavailable(
                "the authenticated UI session did not become ready in time",
            ))
        }
    }
}

fn receive_events<F>(
    receiver: &Receiver<SessionDelivery>,
    on_event: &mut F,
) -> Result<(), UiTransportFailure>
where
    F: FnMut(Value) -> Result<(), UiTransportFailure>,
{
    loop {
        match receiver.recv() {
            Ok(SessionDelivery::Event(event)) => on_event(event)?,
            Ok(SessionDelivery::Failure(failure)) => return Err(failure),
            Ok(SessionDelivery::Closed) => return Ok(()),
            Err(_) => {
                return Err(UiTransportFailure::unavailable(
                    "the Core UI event channel closed without a terminal signal",
                ));
            }
        }
    }
}

fn open_session(
    config: &CorePipeConfig,
    token: &str,
) -> Result<(NamedPipeTransport, ServerHello), UiTransportFailure> {
    let mut transport = NamedPipeTransport::client(&config.pipe_name)
        .map_err(|error| UiTransportFailure::unavailable(error.to_string()))?;
    transport
        .connect(config.connect_timeout)
        .map_err(|error| UiTransportFailure::unavailable(error.to_string()))?;
    let server_hello = client_handshake(&mut transport, token, Vec::new(), config.connect_timeout)
        .map_err(|error| UiTransportFailure::unavailable(error.to_string()))?;
    Ok((transport, server_hello))
}

fn run_worker(
    mut transport: NamedPipeTransport,
    server_hello: &ServerHello,
    command_rx: Receiver<WorkerCommand>,
    event_sink: EventSink,
    is_alive: Arc<AtomicBool>,
    timeouts: WorkerTimeouts,
) {
    let result = run_worker_loop(
        &mut transport,
        server_hello,
        timeouts,
        &command_rx,
        &event_sink,
    );
    match result {
        WorkerExit::Closed => {
            deliver_terminal(&event_sink, SessionDelivery::Closed);
        }
        WorkerExit::Failed(failure) => {
            deliver_terminal(&event_sink, SessionDelivery::Failure(failure));
        }
    }
    is_alive.store(false, Ordering::Release);
}

enum WorkerExit {
    Closed,
    Failed(UiTransportFailure),
}

struct IncomingContext<'a> {
    session_id: &'a str,
    request_timeout: Duration,
    event_sink: &'a EventSink,
}

fn run_worker_loop(
    transport: &mut NamedPipeTransport,
    server_hello: &ServerHello,
    timeouts: WorkerTimeouts,
    command_rx: &Receiver<WorkerCommand>,
    event_sink: &EventSink,
) -> WorkerExit {
    let command_poll = timeouts
        .request
        .min(Duration::from_millis(50))
        .max(Duration::from_millis(10));
    let heartbeat_interval = timeouts
        .request
        .min(Duration::from_secs(1))
        .max(Duration::from_millis(100));
    let mut pending: Option<WorkerCommand> = None;
    let mut human_decision_seen = false;
    let mut last_received = Instant::now();
    let mut last_heartbeat = Instant::now();
    let incoming = IncomingContext {
        session_id: &server_hello.session_id,
        request_timeout: timeouts.request,
        event_sink,
    };

    loop {
        if let Err(failure) = drain_commands(
            transport,
            command_rx,
            &mut pending,
            timeouts.request,
            &mut human_decision_seen,
        ) {
            return failed_exit(pending.take(), failure);
        }
        if let Err(failure) = maintain_heartbeat(
            transport,
            &server_hello.session_id,
            timeouts,
            heartbeat_interval,
            last_received,
            &mut last_heartbeat,
        ) {
            return failed_exit(pending.take(), failure);
        }

        match transport.recv(command_poll) {
            Ok(message) => {
                last_received = Instant::now();
                match handle_incoming(
                    transport,
                    message,
                    &mut pending,
                    &mut human_decision_seen,
                    &incoming,
                ) {
                    Ok(true) => last_heartbeat = Instant::now(),
                    Ok(false) => {}
                    Err(failure) => return failed_exit(pending.take(), failure),
                }
            }
            Err(IpcError::Timeout { .. }) => {}
            Err(IpcError::Disconnected { .. }) if human_decision_seen => {
                return WorkerExit::Closed;
            }
            Err(error) => {
                return failed_exit(
                    pending.take(),
                    UiTransportFailure::unavailable(error.to_string()),
                );
            }
        }
    }
}

fn maintain_heartbeat(
    transport: &mut NamedPipeTransport,
    session_id: &str,
    timeouts: WorkerTimeouts,
    heartbeat_interval: Duration,
    last_received: Instant,
    last_heartbeat: &mut Instant,
) -> Result<(), UiTransportFailure> {
    if last_received.elapsed() > timeouts.heartbeat {
        return Err(UiTransportFailure::unavailable(
            "the Core UI session stopped sending heartbeats",
        ));
    }
    if last_heartbeat.elapsed() >= heartbeat_interval {
        send_heartbeat(transport, session_id, timeouts.request)?;
        *last_heartbeat = Instant::now();
    }
    Ok(())
}

fn handle_incoming(
    transport: &mut NamedPipeTransport,
    message: WireMessage,
    pending: &mut Option<WorkerCommand>,
    human_decision_seen: &mut bool,
    context: &IncomingContext<'_>,
) -> Result<bool, UiTransportFailure> {
    match message {
        WireMessage::Heartbeat(heartbeat) => {
            if heartbeat.session_id != context.session_id || !heartbeat.alive {
                return Err(UiTransportFailure::unavailable(
                    "Core sent a heartbeat for a different or dead UI session",
                ));
            }
            send_heartbeat(transport, context.session_id, context.request_timeout)?;
            Ok(true)
        }
        WireMessage::UiEvent(event) => {
            deliver_event(context.event_sink, event.event)?;
            Ok(false)
        }
        WireMessage::UiResponse(response) => {
            handle_response(response, pending, human_decision_seen)?;
            Ok(false)
        }
        other => Err(UiTransportFailure::unavailable(format!(
            "unexpected {} on the authenticated UI session",
            other.kind()
        ))),
    }
}

fn handle_response(
    response: assistant_ipc::UiIpcResponse,
    pending: &mut Option<WorkerCommand>,
    human_decision_seen: &mut bool,
) -> Result<(), UiTransportFailure> {
    let Some(command) = pending.take() else {
        return Err(UiTransportFailure::unavailable(
            "Core sent a UI response with no command in flight",
        ));
    };
    if response.correlation_id != command.correlation_id {
        let failure =
            UiTransportFailure::unavailable("response correlation_id does not match the request");
        fail_pending(Some(command), &failure);
        return Err(failure);
    }
    *human_decision_seen = response_is_human_decision(&response.result);
    let result = interpret_response(&command.correlation_id, WireMessage::UiResponse(response));
    let _ = command.reply.try_send(result);
    Ok(())
}

fn failed_exit(pending: Option<WorkerCommand>, failure: UiTransportFailure) -> WorkerExit {
    fail_pending(pending, &failure);
    WorkerExit::Failed(failure)
}

fn drain_commands(
    transport: &mut NamedPipeTransport,
    command_rx: &Receiver<WorkerCommand>,
    pending: &mut Option<WorkerCommand>,
    timeout: Duration,
    human_decision_seen: &mut bool,
) -> Result<(), UiTransportFailure> {
    loop {
        match command_rx.try_recv() {
            Ok(command) => {
                if pending.is_some() {
                    let _ = command.reply.try_send(Err(UiTransportFailure::unavailable(
                        "another UI command is already in flight; the command was rejected",
                    )));
                    continue;
                }
                *human_decision_seen = false;
                let request = WireMessage::UiRequest(UiIpcRequest::new(
                    command.correlation_id.clone(),
                    command.envelope.clone(),
                ));
                if let Err(error) = transport.send(&request, timeout) {
                    let failure = UiTransportFailure::unavailable(error.to_string());
                    let _ = command.reply.try_send(Err(failure.clone()));
                    return Err(failure);
                }
                *pending = Some(command);
            }
            Err(TryRecvError::Empty) => return Ok(()),
            Err(TryRecvError::Disconnected) => return Ok(()),
        }
    }
}

fn send_heartbeat(
    transport: &mut NamedPipeTransport,
    session_id: &str,
    timeout: Duration,
) -> Result<(), UiTransportFailure> {
    let timestamp_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            UiTransportFailure::unavailable(format!(
                "system clock is before the Unix epoch: {error}"
            ))
        })?
        .as_millis()
        .try_into()
        .map_err(|_| UiTransportFailure::unavailable("system time does not fit in u64"))?;
    transport
        .send(
            &WireMessage::Heartbeat(Heartbeat {
                session_id: session_id.to_owned(),
                timestamp_unix_ms,
                alive: true,
            }),
            timeout,
        )
        .map_err(|error| UiTransportFailure::unavailable(error.to_string()))
}

fn deliver_event(event_sink: &EventSink, event: Value) -> Result<(), UiTransportFailure> {
    let sink = event_sink
        .lock()
        .map_err(|_| UiTransportFailure::unavailable("the UI event sink lock is unavailable"))?;
    let Some(sender) = sink.as_ref() else {
        return Ok(());
    };
    match sender.try_send(SessionDelivery::Event(event)) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => Err(UiTransportFailure::unavailable(
            "the UI event queue is full; the session was stopped",
        )),
        Err(TrySendError::Disconnected(_)) => Err(UiTransportFailure::unavailable(
            "the UI event consumer is no longer running",
        )),
    }
}

fn deliver_terminal(event_sink: &EventSink, delivery: SessionDelivery) {
    if let Ok(sink) = event_sink.lock()
        && let Some(sender) = sink.as_ref()
    {
        let _ = sender.send(delivery);
    }
}

fn fail_pending(pending: Option<WorkerCommand>, failure: &UiTransportFailure) {
    if let Some(command) = pending {
        let _ = command.reply.try_send(Err(failure.clone()));
    }
}

fn response_is_human_decision(result: &UiIpcResult) -> bool {
    let UiIpcResult::Outcome { outcome } = result else {
        return false;
    };
    matches!(
        outcome.get("status").and_then(Value::as_str),
        Some("approval_granted" | "approval_denied")
    )
}

#[cfg(test)]
#[path = "core_pipe_session_tests.rs"]
mod tests;

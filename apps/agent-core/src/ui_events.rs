//! Production UI event source: task snapshots → UI events (ADR-0057 D5).
//!
//! Responsibilities:
//! - turn the task engine's latest [`TaskSnapshot`] into the structured events
//!   the timeline renders, via [`project_snapshot_events`];
//! - emit each snapshot **once**: events are produced only when the snapshot
//!   revision changes.
//!
//! Boundaries:
//! - does not own the task engine or decide when it is persisted; the assembly
//!   supplies a provider, which is what keeps this module free of locks and of
//!   a second source of truth;
//! - does not render, translate, or filter events (that is the UI's job).
//!
//! Invariants:
//! 1. the same revision never produces events twice (no duplicate timeline rows);
//! 2. a provider returning `None` (no task yet) yields no events and does not
//!    reset the dedup state;
//! 3. event order inside one snapshot is deterministic — it is exactly
//!    `project_snapshot_events`' order.

use std::sync::Arc;

use assistant_task_engine::TaskSnapshot;

use crate::ui_control::PendingApprovals;
use crate::ui_ipc::{UiEvent, project_snapshot_events};
use crate::ui_server::UiEventSource;

/// Emits UI events whenever the provided snapshot's revision changes.
///
/// The provider is the seam the assembly fills: it can read the executor's
/// current snapshot however it likes (a shared handle, a checkpoint read, a
/// test fixture) without this module taking a position on ownership.
pub struct SnapshotEventSource<Provider> {
    provider: Provider,
    last_revision: Option<u64>,
}

impl<Provider> SnapshotEventSource<Provider> {
    /// Creates a source over a snapshot provider.
    #[must_use]
    pub const fn new(provider: Provider) -> Self {
        Self {
            provider,
            last_revision: None,
        }
    }
}

impl<Provider> UiEventSource for SnapshotEventSource<Provider>
where
    Provider: FnMut() -> Option<TaskSnapshot>,
{
    fn drain(&mut self) -> Vec<UiEvent> {
        let Some(snapshot) = (self.provider)() else {
            return Vec::new();
        };
        if self.last_revision == Some(snapshot.revision) {
            return Vec::new();
        }
        self.last_revision = Some(snapshot.revision);
        project_snapshot_events(&snapshot)
    }
}

/// Emits browser-form approval events registered in the shared pending registry.
///
/// The source drains each queued event exactly once. The pending decision stays
/// registered until the UI approves or denies it; approving or denying removes
/// any event that has not been delivered yet.
pub struct PendingApprovalEventSource {
    pending: Arc<PendingApprovals>,
}

impl PendingApprovalEventSource {
    /// Creates a source over the same registry used by the task-control handler.
    #[must_use]
    pub const fn new(pending: Arc<PendingApprovals>) -> Self {
        Self { pending }
    }
}

impl UiEventSource for PendingApprovalEventSource {
    fn drain(&mut self) -> Vec<UiEvent> {
        // The queue contains plain events and has no panic path while locked.
        // Recovering a poisoned lock avoids turning a completed registration
        // into a silent "no event" result.
        self.pending.drain_events_recovering_poison()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Arc;

    use assistant_protocol::serde_json::json;
    use assistant_task_engine::{
        Budget, CheckpointPolicy, MemoryCheckpointStore, Plan, PlanId, PlanStep, Reversibility,
        StepEffect, StepId, StepTimeouts, TaskEngine, TaskEvent, TaskId,
    };

    use super::{PendingApprovalEventSource, SnapshotEventSource};
    use crate::ui_control::PendingApprovals;
    use crate::ui_ipc::UiEvent;
    use crate::ui_server::UiEventSource;

    /// 构造一个处于 `Running` 的引擎；任一步骤失败返回 `None`（测试里不 panic）。
    fn running_engine() -> Option<(TaskEngine<MemoryCheckpointStore>, TaskId)> {
        let task_id = TaskId::new("t_events").ok()?;
        let plan = Plan {
            plan_id: PlanId::new("p_events").ok()?,
            task_id: task_id.clone(),
            goal: "events".to_owned(),
            steps: vec![PlanStep {
                id: StepId::new("s_1").ok()?,
                sequence: 1,
                tool: "notepad.text.write".to_owned(),
                args: json!({ "text": "hello" }),
                depends_on: Vec::new(),
                postconditions: vec![json!({ "kind": "text_contains", "value": "hello" })],
                effect: StepEffect::Write,
                reversibility: Reversibility::L0UndoStack,
                point_of_no_return: false,
                timeouts: StepTimeouts::new(500, 2_000, 1_000).ok()?,
            }],
            budget: Budget::new(10, 60_000, 10_000, 1.0).ok()?,
            checkpoint_policy: CheckpointPolicy::AfterEachTransition,
        };
        let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
        engine.create_task(plan, 1_000).ok()?;
        engine
            .apply_task_event(&task_id, TaskEvent::SubmitForApproval, 1_010)
            .ok()?;
        engine
            .apply_task_event(&task_id, TaskEvent::ApprovePlan, 1_020)
            .ok()?;
        Some((engine, task_id))
    }

    #[test]
    fn test_first_snapshot_produces_events() {
        let Some((engine, task_id)) = running_engine() else {
            return;
        };
        let mut source = SnapshotEventSource::new(move || engine.load_snapshot(&task_id).ok());
        let events = source.drain();
        assert!(!events.is_empty(), "首个快照必须产出事件");
        assert!(matches!(
            events.first(),
            Some(crate::ui_ipc::UiEvent::TaskStateChanged { status, .. }) if status == "running"
        ));
    }

    #[test]
    fn test_same_revision_never_produces_events_twice() {
        let Some((engine, task_id)) = running_engine() else {
            return;
        };
        let mut source = SnapshotEventSource::new(move || engine.load_snapshot(&task_id).ok());
        assert!(!source.drain().is_empty());
        assert!(
            source.drain().is_empty(),
            "同一 revision 不得重复产出事件（否则时间线会出现重复行）"
        );
    }

    #[test]
    fn test_provider_returning_none_yields_no_events_and_keeps_dedup_state() {
        let Some((engine, task_id)) = running_engine() else {
            return;
        };
        let mut calls = 0_u8;
        let mut source = SnapshotEventSource::new(move || {
            calls += 1;
            if calls == 2 {
                return None;
            }
            engine.load_snapshot(&task_id).ok()
        });
        assert!(!source.drain().is_empty());
        assert!(source.drain().is_empty(), "None 不产出事件");
        assert!(source.drain().is_empty(), "None 不得重置去重状态");
    }

    #[test]
    fn test_newer_revision_produces_events_again() {
        let Some((mut engine, task_id)) = running_engine() else {
            return;
        };
        let Ok(step_id) = StepId::new("s_1") else {
            return;
        };
        let mut calls = 0_u8;
        let mut source = SnapshotEventSource::new(move || {
            calls += 1;
            if calls == 2 {
                // 第二次读取前推进一次状态机 —— 产生新的 revision
                engine.begin_step(&task_id, &step_id, 2_000).ok()?;
            }
            engine.load_snapshot(&task_id).ok()
        });
        assert!(!source.drain().is_empty());
        assert!(
            !source.drain().is_empty(),
            "revision 变化后必须重新产出事件"
        );
    }
    #[test]
    fn test_pending_approval_source_drains_each_event_once() {
        let pending = Arc::new(PendingApprovals::new());
        let event: UiEvent = serde_json::from_value(json!({
            "kind": "step_approval_requested",
            "request_id": "runtime:t_events:s_1",
            "task_id": "t_events",
            "step_id": "s_1",
            "action": "edge.cdp.form_submit",
            "target": "http://127.0.0.1:8765",
            "summary": "Submit one changed field",
            "impact": "Submission cannot be automatically undone.",
            "diff": {
                "kind": "fields",
                "entries": [{
                    "field": "#display-name",
                    "before": "",
                    "after": "[redacted:3]"
                }]
            },
            "instruction_attribution": {
                "origin": "user_request",
                "parent_goal": null,
                "source_ref": null
            },
            "risk": "high",
            "reversibility": "l3_irreversible",
            "scope_options": ["once"],
            "evidence": {
                "form_origin": "http://127.0.0.1:8765",
                "form_action": "http://127.0.0.1:8765/submitted"
            },
            "expires_at_ms": 4_102_444_800_000_i64
        }))
        .expect("approval event");
        pending
            .register_event(&event, 1_000)
            .expect("register approval event");

        let mut source = PendingApprovalEventSource::new(Arc::clone(&pending));
        assert_eq!(source.drain(), vec![event]);
        assert!(
            source.drain().is_empty(),
            "an event source must not replay a delivered approval card"
        );
    }
}

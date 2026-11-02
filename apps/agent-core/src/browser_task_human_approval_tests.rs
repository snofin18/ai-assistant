//! Human-approval-specific browser task runner tests.
//!
//! These tests remain under the scripted `ToolBus` boundary and never start a
//! browser, profile, network listener, or GUI.

use super::*;

#[test]
fn test_t5_2_human_approval_package_renders_four_committed_steps()
-> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/browser.edge/tasks/t5.2.profile-human-approval.json");
    let provider = TaskPackageProvider::from_package_file(&path)?;
    let plan: Value = serde_json::from_str(provider.plan_json())?;
    let tools = plan
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| fake_error("human approval plan has no steps"))?
        .iter()
        .filter_map(|step| step.get("tool").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(
        tools,
        vec![
            EDGE_NAVIGATE,
            EDGE_FORM_READ,
            EDGE_FORM_FILL,
            EDGE_FORM_SUBMIT
        ]
    );
    Ok(())
}

#[test]
fn test_browser_task_options_human_approval_allows_non_submit_preapproval() {
    let output = std::env::temp_dir().join(format!(
        "assistant-browser-task-human-approval-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&output);
    let base = vec![
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
    ];
    let mut arguments = base;
    arguments.push("--human-approval".to_owned());
    let options = BrowserTaskOptions::parse(&arguments).expect("human approval options");
    assert!(options.human_approval);
    assert!(options.approved_step_ids.is_empty());

    arguments.extend(["--approve-step".to_owned(), "navigate_form".to_owned()]);
    let options = BrowserTaskOptions::parse(&arguments).expect("non-submit preapproval options");
    assert!(options.human_approval);
    assert_eq!(options.approved_step_ids, vec!["navigate_form"]);
}

#[test]
fn test_human_approval_rejects_form_submit_preapproval() -> Result<(), Box<dyn std::error::Error>> {
    let submit_step = PlanStep {
        id: StepId::new("submit_form")?,
        sequence: 4,
        tool: EDGE_FORM_SUBMIT.to_owned(),
        args: json!({}),
        depends_on: Vec::new(),
        postconditions: Vec::new(),
        effect: StepEffect::Write,
        reversibility: Reversibility::L3Irreversible,
        point_of_no_return: true,
        timeouts: StepTimeouts::new(500, 2_000, 1_000)?,
    };
    let error = reject_form_submit_preapproval(
        std::slice::from_ref(&submit_step),
        std::slice::from_ref(&submit_step.id),
        true,
    )
    .expect_err("human mode must not pre-approve submit");
    assert!(error.to_string().contains("cannot pre-approve form_submit"));
    Ok(())
}

#[test]
fn test_human_approval_grants_exactly_once_and_expires_fail_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let recorder = Arc::new(Mutex::new(BrowserInvocationRecorder::new(
        InstructionAttribution::plan_derived("Fill the local profile form")?,
    )));
    recorder
        .lock()
        .map_err(|_| fake_error("recorder state is poisoned"))?
        .record(EDGE_FORM_READ, "read-form", &runtime_form_read_envelope())?;
    recorder
        .lock()
        .map_err(|_| fake_error("recorder state is poisoned"))?
        .record(EDGE_FORM_FILL, "fill-form", &runtime_form_fill_envelope())?;
    let task_id = TaskId::new("task-human-approval")?;
    let submit_step = PlanStep {
        id: StepId::new("submit-form")?,
        sequence: 4,
        tool: EDGE_FORM_SUBMIT.to_owned(),
        args: json!({}),
        depends_on: Vec::new(),
        postconditions: Vec::new(),
        effect: StepEffect::Write,
        reversibility: Reversibility::L3Irreversible,
        point_of_no_return: true,
        timeouts: StepTimeouts::new(500, 2_000, 1_000)?,
    };
    recorder
        .lock()
        .map_err(|_| fake_error("recorder state is poisoned"))?
        .prepare_form_submit_approval(&task_id, &submit_step, 10_000)?;
    let pending = {
        let recorder = recorder
            .lock()
            .map_err(|_| fake_error("recorder state is poisoned"))?;
        Arc::clone(&recorder.pending_approvals)
    };
    let event = take_runtime_approval_event(&pending, &task_id, &submit_step.id)?;
    let grants = ApprovalGrants::new();
    grant_human_approval(&event, &grants, 10_000)?;
    assert!(matches!(
        grants.consume(task_id.as_str(), submit_step.id.as_str(), 10_001),
        Ok(Some(ApprovalScope::Once))
    ));
    assert!(matches!(
        grants.consume(task_id.as_str(), submit_step.id.as_str(), 10_002),
        Ok(None)
    ));

    let expired = grant_human_approval(&event, &ApprovalGrants::new(), 310_000)
        .expect_err("expired human approval must fail closed");
    assert!(expired.to_string().contains("expired"));
    Ok(())
}

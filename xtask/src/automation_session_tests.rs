//! `automation_session` 的私有白盒测试。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    HumanReset, SessionSource, classify_session_meta, discover_current_session, load_human_reset,
    reset_watermark_path, save_human_reset,
};

fn temp_root(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!(
        "xtask-automation-session-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&path)?;
    Ok(path)
}

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self(temp_root(label)?))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_classify_session_meta_keeps_human_and_automation_distinct() {
    assert_eq!(
        classify_session_meta(r#"{"thread_source":"automation"}"#),
        Some(SessionSource::Automation)
    );
    assert_eq!(
        classify_session_meta(r#"{ "thread_source" : "agent_created_thread" }"#),
        Some(SessionSource::Human)
    );
    assert_eq!(classify_session_meta(r#"{"source":"vscode"}"#), None);
}

#[test]
fn test_discover_current_session_reads_only_meta_prefix() -> Result<(), Box<dyn std::error::Error>>
{
    let root = TempRoot::new("discover")?;
    let session_dir = root.path().join("sessions/2026/10/29");
    fs::create_dir_all(&session_dir)?;
    let session_id = "01a-test-human";
    let session_file = session_dir.join(format!("rollout-2026-10-29T08-00-00-{session_id}.jsonl"));
    fs::write(
        &session_file,
        "{\"type\":\"session_meta\",\"payload\":{\"thread_source\":\"agent_created_thread\"}}\n{\"role\":\"user\"}\n",
    )?;
    let session = discover_current_session(root.path(), session_id)?;
    assert_eq!(session.source, SessionSource::Human);
    assert_eq!(session.session_file, session_file);
    Ok(())
}

#[test]
fn test_discover_current_session_rejects_unknown_source() -> Result<(), Box<dyn std::error::Error>>
{
    let root = TempRoot::new("unknown")?;
    let session_dir = root.path().join("sessions/2026/10/29");
    fs::create_dir_all(&session_dir)?;
    let session_id = "01a-test-unknown";
    fs::write(
        session_dir.join(format!("rollout-{session_id}.jsonl")),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"x\"}}\n",
    )?;
    assert!(discover_current_session(root.path(), session_id).is_err());
    Ok(())
}

#[test]
fn test_discover_current_session_rejects_duplicate_rollouts()
-> Result<(), Box<dyn std::error::Error>> {
    let root = TempRoot::new("duplicate")?;
    let first = root.path().join("sessions/2026/10/29/a");
    let second = root.path().join("sessions/2026/10/30/b");
    fs::create_dir_all(&first)?;
    fs::create_dir_all(&second)?;
    let session_id = "01a-test-duplicate";
    let meta =
        "{\"type\":\"session_meta\",\"payload\":{\"thread_source\":\"agent_created_thread\"}}\n";
    fs::write(first.join(format!("rollout-a-{session_id}.jsonl")), meta)?;
    fs::write(second.join(format!("rollout-b-{session_id}.jsonl")), meta)?;
    assert!(discover_current_session(root.path(), session_id).is_err());
    Ok(())
}

#[test]
fn test_human_reset_round_trips_as_single_file() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempRoot::new("reset-roundtrip")?;
    let reset = HumanReset {
        session_id: "01a-test-human".to_owned(),
        recorded_at_epoch_ms: 1_793_250_000_000,
        after_run_date: Some("2026-10-28".to_owned()),
        after_round: 5,
    };
    let path = save_human_reset(root.path(), &reset)?;
    assert_eq!(path, reset_watermark_path(root.path()));
    assert_eq!(load_human_reset(root.path())?, Some(reset));
    assert_eq!(
        fs::read_dir(root.path().join("automations"))?.count(),
        1,
        "reset 水位必须是单文件，不得无限累积"
    );
    Ok(())
}

#[test]
fn test_human_reset_rejects_corrupt_file() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempRoot::new("reset-corrupt")?;
    let path = reset_watermark_path(root.path());
    fs::create_dir_all(path.parent().ok_or("parent")?)?;
    fs::write(&path, "schema_version=1\nsession_id=broken\n")?;
    assert!(load_human_reset(root.path()).is_err());
    Ok(())
}

#[test]
fn test_human_reset_rejects_oversized_file() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempRoot::new("reset-oversized")?;
    let path = reset_watermark_path(root.path());
    fs::create_dir_all(path.parent().ok_or("parent")?)?;
    fs::write(&path, "x".repeat(5_000))?;
    assert!(load_human_reset(root.path()).is_err());
    Ok(())
}

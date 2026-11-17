//! `automation_rounds` 的私有白盒测试。
#![allow(clippy::expect_used, clippy::indexing_slicing)]

use super::{
    Mechanism, ResetSource, RoundEntry, classify_mechanism, is_git_environment_name,
    load_main_round_files, parse_round_file_name, run_git_command, summarize,
    summarize_with_human_reset, validate_run_date,
};
use crate::automation_session::{CurrentSession, HumanReset, SessionSource};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn entry(path: &str, run_date: &str, round: u32, mechanism: Mechanism) -> RoundEntry {
    RoundEntry {
        path: path.to_owned(),
        run_date: run_date.to_owned(),
        round,
        mechanism,
    }
}

#[test]
fn test_parse_round_file_name_accepts_expected_shape() {
    let parsed = parse_round_file_name("2026-10-24-round-12.md");
    assert_eq!(parsed, Some(("2026-10-24".to_owned(), 12)));
}

#[test]
fn test_parse_round_file_name_rejects_non_round_shapes() {
    for value in [
        "2026-10-24-report.md",
        "2026-10-24-round-.md",
        "2026-10-24-round-0.md",
        "2026-10-24-round-1x.md",
        "2026-1-24-round-1.md",
    ] {
        assert_eq!(parse_round_file_name(value), None, "{value}");
    }
}

#[test]
fn test_validate_run_date_rejects_wrong_shapes() {
    assert!(validate_run_date("2026-10-26").is_ok());
    for value in ["2026-1-26", "2026/10/26", "20261026", "2026-10-2x"] {
        assert!(validate_run_date(value).is_err(), "{value}");
    }
}

#[test]
fn test_classify_mechanism_reads_human_declaration() {
    let content = "# round\n\n- **机制**：human（人类会话）\n";
    assert_eq!(classify_mechanism(content), Mechanism::Human);
}

#[test]
fn test_classify_mechanism_reads_automation_declaration() {
    let content = "# round\n\n- **机制**：automation（无人值守）\n";
    assert_eq!(classify_mechanism(content), Mechanism::Automation);
}

#[test]
fn test_classify_mechanism_reads_title_declaration() {
    let content = "# 2026-10-14 round 5（机制：automation）\n";
    assert_eq!(classify_mechanism(content), Mechanism::Automation);
}

#[test]
fn test_classify_mechanism_ignores_body_prose() {
    let content = "# round\n\n正文讨论 human 与 automation，但没有声明。\n";
    assert_eq!(classify_mechanism(content), Mechanism::Missing);
}

#[test]
fn test_summary_counts_automation_after_latest_human() {
    let entries = vec![
        entry("h1", "2026-10-05", 1, Mechanism::Human),
        entry("a1", "2026-10-05", 2, Mechanism::Automation),
        entry("m1", "2026-10-05", 3, Mechanism::Missing),
    ];
    let summary = summarize(&entries, "2026-10-05");
    assert_eq!(summary.latest_human.as_deref(), Some("h1"));
    assert_eq!(summary.n_auto, 2);
    assert_eq!(summary.missing_after_latest_human, vec!["m1"]);
    assert!(
        !summary.is_usable(),
        "missing 必须按 automation fail-closed"
    );
}

#[test]
fn test_daily_boundary_counts_only_current_run_date() {
    let entries = vec![entry("a1", "2026-10-02", 1, Mechanism::Automation)];
    let summary = summarize(&entries, "2026-10-02");
    assert_eq!(summary.latest_human.as_deref(), Some("daily:2026-10-02"));
    assert_eq!(summary.reset_source, ResetSource::DailyBoundary);
    assert_eq!(summary.n_auto, 1);
    assert!(summary.is_usable());
}

#[test]
fn test_summary_computes_round_for_requested_date() {
    let entries = vec![
        entry("r1", "2026-10-26", 1, Mechanism::Human),
        entry("r2", "2026-10-26", 2, Mechanism::Automation),
        entry("other", "2026-10-25", 8, Mechanism::Automation),
    ];
    let summary = summarize(&entries, "2026-10-26");
    assert_eq!(summary.existing_rounds_for_run_date, 2);
    assert_eq!(summary.next_round, 3);
    assert_eq!(summary.n_auto, 1);
    assert!(summary.is_usable());
}

#[test]
fn test_historic_missing_before_human_is_warning_only() {
    let entries = vec![
        entry("old-missing", "2026-10-02", 1, Mechanism::Missing),
        entry("human", "2026-10-02", 2, Mechanism::Human),
        entry("automation", "2026-10-02", 3, Mechanism::Automation),
    ];
    let summary = summarize(&entries, "2026-10-02");
    assert_eq!(summary.missing_before_latest_human, vec!["old-missing"]);
    assert!(summary.missing_after_latest_human.is_empty());
    assert_eq!(summary.n_auto, 1);
    assert!(summary.is_usable());
}

#[test]
fn test_human_session_reset_counts_only_later_automation() {
    let entries = vec![
        entry("old", "2026-10-04", 1, Mechanism::Automation),
        entry("later-a", "2026-10-04", 2, Mechanism::Automation),
        entry("later-missing", "2026-10-04", 3, Mechanism::Missing),
    ];
    let reset = HumanReset {
        session_id: "human-session".to_owned(),
        recorded_at_epoch_ms: 1,
        after_run_date: Some("2026-10-04".to_owned()),
        after_round: 1,
    };
    let summary = summarize_with_human_reset(&entries, "2026-10-04", Some(&reset), None);
    assert_eq!(
        summary.latest_human.as_deref(),
        Some("session:human-session")
    );
    assert_eq!(summary.reset_source, ResetSource::HumanSession);
    assert_eq!(summary.n_auto, 2);
    assert_eq!(summary.missing_after_latest_human, vec!["later-missing"]);
    assert!(!summary.is_usable());
}

#[test]
fn test_human_session_after_all_rounds_resets_to_zero() {
    let entries = vec![
        entry("a1", "2026-10-01", 1, Mechanism::Automation),
        entry("a2", "2026-10-03", 1, Mechanism::Automation),
    ];
    let reset = HumanReset {
        session_id: "human-session".to_owned(),
        recorded_at_epoch_ms: 1,
        after_run_date: Some("2026-10-03".to_owned()),
        after_round: 1,
    };
    let current = CurrentSession {
        session_id: "human-session".to_owned(),
        source: SessionSource::Human,
        session_file: PathBuf::from("session.jsonl"),
    };
    let summary = summarize_with_human_reset(&entries, "2026-10-03", Some(&reset), Some(&current));
    assert_eq!(summary.n_auto, 0);
    assert_eq!(summary.reset_source, ResetSource::HumanSession);
    assert!(summary.is_usable());
}

#[test]
fn test_later_human_round_wins_over_older_session_watermark() {
    let entries = vec![
        entry("a1", "2026-10-02", 1, Mechanism::Automation),
        entry("h2", "2026-10-04", 1, Mechanism::Human),
        entry("a3", "2026-10-04", 2, Mechanism::Automation),
    ];
    let reset = HumanReset {
        session_id: "old-human-session".to_owned(),
        recorded_at_epoch_ms: 1,
        after_run_date: Some("2026-10-02".to_owned()),
        after_round: 1,
    };
    let summary = summarize_with_human_reset(&entries, "2026-10-04", Some(&reset), None);
    assert_eq!(summary.n_auto, 1);
    assert_eq!(summary.reset_source, ResetSource::HumanRound);
    assert_eq!(summary.latest_human.as_deref(), Some("h2"));
}

#[test]
fn test_no_round_reset_falls_back_to_daily_boundary() {
    let entries = vec![entry("a1", "2026-10-01", 1, Mechanism::Automation)];
    let reset = HumanReset {
        session_id: "first-human-session".to_owned(),
        recorded_at_epoch_ms: 1,
        after_run_date: None,
        after_round: 0,
    };
    let summary = summarize_with_human_reset(&entries, "2026-10-01", Some(&reset), None);
    assert_eq!(summary.n_auto, 1);
    assert_eq!(summary.reset_source, ResetSource::DailyBoundary);
    assert!(summary.is_usable());
}

#[test]
fn test_human_prompt_turn_boundary_wins_over_daily_boundary() {
    let entries = vec![
        entry("a1", "2026-10-29", 1, Mechanism::Automation),
        entry("a2", "2026-10-29", 2, Mechanism::Automation),
    ];
    let reset = HumanReset {
        session_id: "same-human-thread".to_owned(),
        recorded_at_epoch_ms: 2,
        after_run_date: Some("2026-10-29".to_owned()),
        after_round: 2,
    };
    let summary = summarize_with_human_reset(&entries, "2026-10-29", Some(&reset), None);
    assert_eq!(summary.reset_source, ResetSource::HumanSession);
    assert_eq!(summary.n_auto, 0);
    assert!(summary.is_usable());
}

#[test]
fn test_daily_boundary_wins_when_human_reset_is_from_previous_day() {
    let entries = vec![
        entry("old", "2026-10-28", 5, Mechanism::Automation),
        entry("today", "2026-10-29", 1, Mechanism::Automation),
    ];
    let reset = HumanReset {
        session_id: "yesterday-human".to_owned(),
        recorded_at_epoch_ms: 1,
        after_run_date: Some("2026-10-28".to_owned()),
        after_round: 5,
    };
    let summary = summarize_with_human_reset(&entries, "2026-10-29", Some(&reset), None);
    assert_eq!(summary.reset_source, ResetSource::DailyBoundary);
    assert_eq!(summary.n_auto, 1);
    assert!(summary.is_usable());
}

struct TempGitRepo {
    root: PathBuf,
}

impl TempGitRepo {
    fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "xtask-automation-rounds-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("clean stale temporary git repository");
        }
        std::fs::create_dir_all(root.join("docs/automations"))
            .expect("create temporary round directory");
        let repo = Self { root };
        repo.git(&["init", "-q"]);
        repo.git(&["config", "user.name", "Task 379 Test"]);
        repo.git(&["config", "user.email", "task379@example.invalid"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo
    }

    fn git(&self, arguments: &[&str]) -> String {
        let output = run_git_command(&self.root, arguments, 1024 * 1024).expect("run git");
        assert!(
            output.status.success(),
            "git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn write_round(&self, file_name: &str, mechanism: &str) {
        let path = self.root.join("docs/automations").join(file_name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create round parent directory");
        }
        std::fs::write(path, format!("# round\n\n- 机制：{mechanism}（测试）\n"))
            .expect("write round file");
    }

    fn commit_all(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }

    fn assert_main_len(&self, expected: usize, context: &str) {
        assert_eq!(
            load_main_round_files(&self.root).expect(context).len(),
            expected
        );
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.root) {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::NotFound,
                "cleanup failed: {error}"
            );
        }
    }
}

#[test]
fn test_main_source_ignores_worktree_index_and_unmerged_branch_only_files() {
    let repo = TempGitRepo::new();
    repo.write_round("2026-11-16-round-1.md", "automation");
    repo.write_round("archive/2026-11-16-round-99.md", "automation");
    repo.write_round("2026-09-26-round-1-run1415.md", "automation");
    repo.write_round("2026-11-02-round-5-skipped.md", "automation");
    repo.write_round("2026-09-26-report.md", "automation");
    repo.write_round("scheduler-acceptance-test.md", "automation");
    repo.commit_all("merged round");
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);

    let merged = load_main_round_files(&repo.root).expect("load merged round");
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].mechanism, Mechanism::Automation);

    repo.write_round("2026-11-16-round-2.md", "automation");
    repo.assert_main_len(1, "untracked file must not affect main");

    repo.git(&["add", "docs/automations/2026-11-16-round-2.md"]);
    repo.assert_main_len(1, "staged file must not affect main");

    repo.commit_all("branch-only round");
    repo.assert_main_len(1, "branch-only file must not affect main");

    repo.git(&["rm", "docs/automations/2026-11-16-round-1.md"]);
    repo.commit_all("branch-only deletion");
    repo.assert_main_len(1, "branch-only deletion must not affect main");
}

#[test]
fn test_main_fallback_uses_local_main_only_when_origin_main_is_absent() {
    let repo = TempGitRepo::new();
    repo.write_round("2026-11-16-round-1.md", "automation");
    repo.commit_all("local main round");
    repo.git(&["update-ref", "refs/heads/main", "HEAD"]);

    repo.write_round("2026-11-16-round-2.md", "automation");
    repo.commit_all("origin main round");
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    repo.assert_main_len(2, "origin/main must win over local main");

    let tree = repo.git(&["show", "-s", "--format=%T", "HEAD"]);
    repo.git(&["update-ref", "refs/remotes/origin/main", tree.trim()]);
    let error = load_main_round_files(&repo.root)
        .expect_err("non-commit origin/main must not fall back to local main");
    assert!(
        error.contains("no usable main ref refs/remotes/origin/main"),
        "{error}"
    );
    repo.git(&["update-ref", "-d", "refs/remotes/origin/main"]);

    repo.assert_main_len(1, "fallback to local main");
}

#[test]
fn test_malformed_round_candidate_fails_closed() {
    let repo = TempGitRepo::new();
    repo.write_round("2026-11-16-round-6-typo.md", "automation");
    repo.commit_all("malformed round");
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);

    let error = load_main_round_files(&repo.root).expect_err("malformed round must fail closed");
    assert!(error.contains("unexpected file"), "{error}");
}

#[test]
fn test_unexpected_non_round_file_fails_closed() {
    for file_name in [
        "notes.md",
        "2026-11-16-report-2.md",
        "2026-11-16-round-0-skipped.md",
    ] {
        let repo = TempGitRepo::new();
        repo.write_round(file_name, "automation");
        repo.commit_all("unexpected document");
        repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);

        let error = load_main_round_files(&repo.root).expect_err("unknown file must fail closed");
        assert!(error.contains("unexpected file"), "{file_name}: {error}");
    }
}

#[test]
fn test_git_environment_name_is_case_insensitive() {
    for name in ["GIT_DIR", "git_dir", "Git_Work_Tree"] {
        assert!(is_git_environment_name(OsStr::new(name)), "{name}");
    }
    assert!(!is_git_environment_name(OsStr::new("PATH")));
}

#[test]
fn test_git_stdout_limit_fails_closed() {
    let repo = TempGitRepo::new();
    let error = run_git_command(&repo.root, &["--version"], 1)
        .expect_err("over-limit git output must fail closed");
    assert!(error.contains("pipe output exceeded 1 bytes"), "{error}");
}

#[test]
fn test_missing_main_refs_fail_closed_without_worktree_fallback() {
    let repo = TempGitRepo::new();
    repo.write_round("2026-11-16-round-1.md", "automation");

    let error = load_main_round_files(&repo.root).expect_err("missing main must fail closed");
    assert!(error.contains("no usable main ref"), "{error}");
}

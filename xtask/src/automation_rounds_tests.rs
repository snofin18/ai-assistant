//! `automation_rounds` 的私有白盒测试。

use super::{
    Mechanism, ResetSource, RoundEntry, classify_mechanism, parse_round_file_name, summarize,
    summarize_with_human_reset, validate_run_date,
};
use crate::automation_session::{CurrentSession, HumanReset, SessionSource};
use std::path::PathBuf;

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
        entry("h1", "2026-10-01", 1, Mechanism::Human),
        entry("a1", "2026-10-02", 1, Mechanism::Automation),
        entry("h2", "2026-10-03", 1, Mechanism::Human),
        entry("a2", "2026-10-04", 1, Mechanism::Automation),
        entry("m1", "2026-10-05", 1, Mechanism::Missing),
    ];
    let summary = summarize(&entries, "2026-10-06");
    assert_eq!(summary.latest_human.as_deref(), Some("h2"));
    assert_eq!(summary.n_auto, 2);
    assert_eq!(summary.missing_after_latest_human, vec!["m1"]);
    assert!(
        !summary.is_usable(),
        "missing 必须按 automation fail-closed"
    );
}

#[test]
fn test_summary_without_human_is_not_usable() {
    let entries = vec![entry("a1", "2026-10-02", 1, Mechanism::Automation)];
    let summary = summarize(&entries, "2026-10-02");
    assert_eq!(summary.latest_human, None);
    assert_eq!(summary.n_auto, 0);
    assert!(!summary.is_usable());
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
        entry("old-missing", "2026-10-01", 1, Mechanism::Missing),
        entry("human", "2026-10-02", 1, Mechanism::Human),
        entry("automation", "2026-10-03", 1, Mechanism::Automation),
    ];
    let summary = summarize(&entries, "2026-10-04");
    assert_eq!(summary.missing_before_latest_human, vec!["old-missing"]);
    assert!(summary.missing_after_latest_human.is_empty());
    assert_eq!(summary.n_auto, 1);
    assert!(summary.is_usable());
}

#[test]
fn test_human_session_reset_counts_only_later_automation() {
    let entries = vec![
        entry("old", "2026-10-01", 1, Mechanism::Automation),
        entry("later-a", "2026-10-03", 1, Mechanism::Automation),
        entry("later-missing", "2026-10-04", 1, Mechanism::Missing),
    ];
    let reset = HumanReset {
        session_id: "human-session".to_owned(),
        recorded_at_epoch_ms: 1,
        after_run_date: Some("2026-10-02".to_owned()),
        after_round: 5,
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
        entry("a2", "2026-10-02", 1, Mechanism::Automation),
    ];
    let reset = HumanReset {
        session_id: "human-session".to_owned(),
        recorded_at_epoch_ms: 1,
        after_run_date: Some("2026-10-02".to_owned()),
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
        entry("a1", "2026-10-01", 1, Mechanism::Automation),
        entry("h2", "2026-10-04", 1, Mechanism::Human),
        entry("a3", "2026-10-05", 1, Mechanism::Automation),
    ];
    let reset = HumanReset {
        session_id: "old-human-session".to_owned(),
        recorded_at_epoch_ms: 1,
        after_run_date: Some("2026-10-02".to_owned()),
        after_round: 1,
    };
    let summary = summarize_with_human_reset(&entries, "2026-10-05", Some(&reset), None);
    assert_eq!(summary.n_auto, 1);
    assert_eq!(summary.reset_source, ResetSource::HumanRound);
    assert_eq!(summary.latest_human.as_deref(), Some("h2"));
}

#[test]
fn test_session_reset_with_no_rounds_counts_all_subsequent_automation() {
    let entries = vec![entry("a1", "2026-10-01", 1, Mechanism::Automation)];
    let reset = HumanReset {
        session_id: "first-human-session".to_owned(),
        recorded_at_epoch_ms: 1,
        after_run_date: None,
        after_round: 0,
    };
    let summary = summarize_with_human_reset(&entries, "2026-10-02", Some(&reset), None);
    assert_eq!(summary.n_auto, 1);
    assert_eq!(summary.reset_source, ResetSource::HumanSession);
    assert!(summary.is_usable());
}

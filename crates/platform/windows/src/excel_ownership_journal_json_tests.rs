//! JSON-shape tests for the platform-private Excel ownership journal.

use super::*;

#[test]
fn test_journal_json_has_exact_fields_and_escapes_strings() {
    let directory = TestDirectory::new();
    let store = ExcelOwnershipJournalStore::existing(directory.path.clone()).expect("store");
    let mut journal = store
        .publish(publication(
            ExcelOwnershipJournalPhase::Opening,
            9,
            Some(Path::new("C:\\data\\quote\"line\nbook.xlsx")),
            &identity(42),
        ))
        .expect("journal");
    let content = fs::read_to_string(journal.path()).expect("journal content");
    let parsed = crate::cdp::parse(content.as_bytes()).expect("valid JSON");
    let object = parsed.as_object().expect("journal object");
    let expected_fields = [
        "schema_version",
        "coordinator_process_id",
        "runner_id",
        "session_id",
        "phase",
        "process_name",
        "process_id",
        "start_time_100ns",
        "executable_path",
        "workbook_path",
    ];
    assert_eq!(object.len(), expected_fields.len());
    for field in expected_fields {
        assert!(object.contains_key(field), "missing field {field}");
    }
    assert_eq!(
        parsed.get("workbook_path").and_then(JsonValue::as_str),
        Some("C:\\data\\quote\"line\nbook.xlsx")
    );
    assert_eq!(
        parsed.get("executable_path").and_then(JsonValue::as_str),
        Some(r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE")
    );
    journal.complete().expect("complete");
    directory.cleanup();
}

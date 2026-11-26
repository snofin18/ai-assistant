//! JSON-shape tests for the platform-private Excel ownership journal.

use super::*;
use crate::cdp::JsonValue;
use std::error::Error;
use std::io;

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "assistant-task411-json-{}",
            NEXT_JOURNAL_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    fn cleanup(self) -> io::Result<()> {
        fs::remove_dir_all(&self.path)
    }
}

#[test]
fn test_journal_json_has_exact_fields_and_escapes_strings() -> Result<(), Box<dyn Error>> {
    let directory = TestDirectory::new()?;
    let store = ExcelOwnershipJournalStore::existing(directory.path.clone())?;
    let identity = ExcelOwnedProcessIdentity::new(
        42,
        420,
        r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE".to_owned(),
    )?;
    let mut journal = store.publish(ExcelOwnershipJournalPublication {
        phase: ExcelOwnershipJournalPhase::Opening,
        runner_id: 1,
        session_id: 9,
        workbook_path: Some(Path::new("C:\\data\\quote\"line\nbook.xlsx")),
        identity: &identity,
    })?;
    let content = fs::read_to_string(journal.path())?;
    let parsed = crate::cdp::parse(content.as_bytes())?;
    let object = parsed
        .as_object()
        .ok_or_else(|| io::Error::other("journal is not a JSON object"))?;
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
    journal.complete()?;
    directory.cleanup()?;
    Ok(())
}

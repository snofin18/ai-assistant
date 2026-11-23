//! Excel COM capability-floor regressions for Adapter validation.
//!
//! Responsibility: prove that process-open and spreadsheet-write capabilities
//! cannot be hidden behind read or low-risk Tool envelopes.
//!
//! Boundary: these tests use the existing Adapter fixture and do not launch
//! Excel, create COM objects, or touch user workbooks.

use super::tests::Fixture;

#[test]
fn test_adapter_validate_rejects_read_tool_claiming_workbook_open() {
    let fixture = Fixture::create("workbook-open-read");
    let path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&path).expect("read tools").replace(
        "\"capabilities\": [\"target.list\"]",
        "\"capabilities\": [\"com.workbook_open\"]",
    );
    std::fs::write(path, tools).expect("write tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("com.workbook_open requires effect=write"),
        "{output}"
    );
    assert!(
        output.contains("com.workbook_open requires l3_irreversible"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_rejects_mixed_workbook_open_capabilities() {
    let fixture = Fixture::create("workbook-open-mixed");
    let path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&path).expect("read tools").replace(
        "\"capabilities\": [\"target.list\"]",
        "\"capabilities\": [\"com.range_write\", \"com.workbook_open\"]",
    );
    std::fs::write(path, tools).expect("write tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("com.workbook_open must be the sole capability"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_rejects_excel_write_capabilities_on_read_tool() {
    for capability in [
        "com.range_write",
        "com.workbook_save",
        "com.chart_create",
        "com.chart_export",
    ] {
        let label = capability.replace('.', "-");
        let fixture = Fixture::create(&label);
        let path = fixture.adapter.join("tools/tools.json");
        let tools = std::fs::read_to_string(&path).expect("read tools").replace(
            "\"capabilities\": [\"target.list\"]",
            &format!("\"capabilities\": [\"{capability}\"]"),
        );
        std::fs::write(path, tools).expect("write tools");
        let (code, output) = fixture.validate();
        assert_eq!(code, 1, "{capability}");
        assert!(
            output.contains(&format!("{capability} requires effect=write")),
            "{capability}: {output}"
        );
    }
}

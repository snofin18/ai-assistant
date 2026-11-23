//! Integration fixtures for `adapter-validate --report-only`.

use super::*;

pub(super) struct Fixture {
    root: PathBuf,
    pub(super) adapter: PathBuf,
}

impl Fixture {
    pub(super) fn create(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "xtask-adapter-validate-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("remove stale fixture");
        }
        let adapter = root.join("adapters/test");
        std::fs::create_dir_all(adapter.join("selectors")).expect("selectors dir");
        std::fs::create_dir_all(adapter.join("tools")).expect("tools dir");
        std::fs::create_dir_all(adapter.join("rollback")).expect("rollback dir");
        std::fs::create_dir_all(adapter.join("interrupts")).expect("interrupts dir");
        std::fs::create_dir_all(adapter.join("memory")).expect("memory dir");
        let catalog_source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join(adapter_catalog::CATALOG_PATH);
        let catalog_target = root.join(adapter_catalog::CATALOG_PATH);
        std::fs::create_dir_all(catalog_target.parent().expect("catalog parent"))
            .expect("catalog dir");
        std::fs::copy(catalog_source, catalog_target).expect("copy catalog");
        let fixture = Self { root, adapter };
        fixture.write_valid();
        fixture
    }

    fn write_valid(&self) {
        std::fs::write(
            self.adapter.join("adapter.toml"),
            concat!(
                "schema_version = 1\n",
                "[adapter]\n",
                "app_id = \"com.example.test\"\n",
                "[bundle]\n",
                "app_map = \"app_map.json\"\n",
                "app_map_projection = \"memory/app_map.v1.json\"\n",
                "app_map_projection_model = \"core_app_map_v1\"\n",
                "selectors = \"selectors/targets.json\"\n",
                "tools = \"tools/tools.json\"\n",
                "rollback = \"rollback/recipes.json\"\n",
                "interrupts = \"interrupts/interrupts.json\"\n",
            ),
        )
        .expect("write adapter.toml");
        std::fs::write(
            self.adapter.join("app_map.json"),
            r#"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "version_range": ">=1 <2",
  "identity": "test app",
  "projection_manifest": {
    "model": "core_app_map_v1",
    "entries": [
      {
        "id": "identity",
        "title": "Identity",
        "content": "test app",
        "source_reference": "bundle:app_map#/identity",
        "token_estimate": 2
      }
    ],
    "omissions": []
  }
}
"#,
        )
        .expect("write app map");
        let app_map_text =
            std::fs::read_to_string(self.adapter.join("app_map.json")).expect("read app map");
        let app_map = serde_json_lite::parse(&app_map_text).expect("parse app map");
        let entries = app_map
            .get("projection_manifest")
            .and_then(|manifest| manifest.get("entries"))
            .and_then(Value::as_array)
            .expect("entries");
        let projection = app_map::canonical_projection_manifest(entries, "com.example.test");
        std::fs::write(self.adapter.join("memory/app_map.v1.json"), projection)
            .expect("write projection");
        std::fs::write(
            self.adapter.join("selectors/targets.json"),
            r#"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "selector_model": "uia_candidate_chain_v1",
  "app_version_range": ">=1 <2",
  "probe_status": "observed",
  "policy": {
    "on_ambiguous": "error_and_ask",
    "on_not_found": {"retry_after_ms": [], "then_escalate": true},
    "min_score_to_try": 0.1,
    "max_resolve_ms": 3000
  },
  "targets": [
    {
      "id": "main",
      "scope": "window",
      "probe_status": "observed",
      "allow_fallback_entry": false,
      "candidates": [
        {
          "id": "main-aid",
          "kind": "automation_id",
          "value": {"text": "MainWindow"},
          "score": 1.0,
          "locale_dependent": false,
          "fallback_only": false
        }
      ]
    }
  ]
}
"#,
        )
        .expect("write selectors");
        std::fs::write(
            self.adapter.join("tools/tools.json"),
            r#"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "tools": [
    {
      "schema": {
        "version": "1.1",
        "name": "test.window.list",
        "description": "List windows.",
        "input": {"type": "object", "properties": {}, "required": [], "additionalProperties": false},
        "output": {"type": "object", "properties": {}, "required": [], "additionalProperties": false},
        "effect": "read",
        "reversibility": "none_readonly",
        "risk_level": "low",
        "requires_approval": false,
        "idempotent": true,
        "tags": ["test"]
      },
      "capabilities": ["target.list"],
      "postconditions": [
        {
          "kind": "state_unchanged",
          "fingerprint_scope": "whole_window",
          "description": "The window set is unchanged.",
          "error_code": "VerifyFailed"
        }
      ]
    }
  ]
}
"#,
        )
        .expect("write tools");
        std::fs::write(
            self.adapter.join("rollback/recipes.json"),
            "{\n  \"schema_version\": 1,\n  \"app_id\": \"com.example.test\",\n  \"recipes\": []\n}\n",
        )
        .expect("write rollback");
        std::fs::write(
            self.adapter.join("interrupts/interrupts.json"),
            "{\n  \"schema_version\": 1,\n  \"app_id\": \"com.example.test\",\n  \"default_action\": \"cancel\",\n  \"interrupts\": []\n}\n",
        )
        .expect("write interrupts");
    }

    fn write_created_tab_bundle(&self) {
        self.write_created_tab_tools();
        self.write_created_tab_rollback();
    }

    fn write_created_tab_tools(&self) {
        std::fs::write(
            self.adapter.join("tools/tools.json"),
            r#"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "tools": [
    {
      "schema": {
        "version": "1.1",
        "name": "test.tab.read_state",
        "description": "Read tab state.",
        "input": {"type": "object", "properties": {}, "required": [], "additionalProperties": false},
        "output": {"type": "object", "properties": {}, "required": [], "additionalProperties": false},
        "effect": "read",
        "reversibility": "none_readonly",
        "risk_level": "low",
        "requires_approval": false,
        "idempotent": true,
        "tags": ["tab"]
      },
      "capabilities": ["element.read_text"],
      "postconditions": [
        {
          "kind": "state_unchanged",
          "fingerprint_scope": "whole_window",
          "description": "Reading tab state does not change the window.",
          "error_code": "VerifyFailed"
        }
      ]
    },
    {
      "schema": {
        "version": "1.1",
        "name": "test.tab.new",
        "description": "Create a new tab.",
        "input": {"type": "object", "properties": {}, "required": [], "additionalProperties": false},
        "output": {"type": "object", "properties": {}, "required": [], "additionalProperties": false},
        "effect": "write",
        "reversibility": "l2_compensation",
        "risk_level": "low",
        "requires_approval": false,
        "idempotent": false,
        "tags": ["new", "tab"]
      },
      "capabilities": ["input.keyboard"],
      "postconditions": [
        {
          "kind": "state_changed",
          "within_ms": 1000,
          "description": "Creating a tab changes the window.",
          "error_code": "VerifyFailed"
        }
      ]
    }
  ]
}
"#,
        )
        .expect("write created-tab tools");
    }

    fn write_created_tab_rollback(&self) {
        std::fs::write(
            self.adapter.join("rollback/recipes.json"),
            r#"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "recipes": [
    {
      "tool": "test.tab.new",
      "coverage": "recipe",
      "anchor_strategy": [
        {
          "level": "l2",
          "mechanism": "compensating_action",
          "capture": {
            "kind": "compensating_state",
            "source": "tool:test.tab.read_state",
            "observations": ["whole_window_fingerprint"]
          },
          "restore": {
            "kind": "close_created_target",
            "target_kind": "tab",
            "require_empty": true
          },
          "post_verify": {
            "kind": "capture_equals",
            "observation": "whole_window_fingerprint"
          }
        }
      ],
      "on_rollback_failure": "publish_incident_and_require_human"
    }
  ]
}
"#,
        )
        .expect("write created-tab rollback");
    }

    pub(super) fn validate(&self) -> (u8, String) {
        let mut output = Vec::new();
        let code = run(&self.root, "adapters/test", true, &mut output).expect("validator run");
        (code, String::from_utf8(output).expect("utf8 output"))
    }

    fn append_postcondition(&self, postcondition: &str) {
        let path = self.adapter.join("tools/tools.json");
        let tools = std::fs::read_to_string(&path).expect("read tools");
        let marker = "          \"error_code\": \"VerifyFailed\"\n        }\n      ]";
        assert!(
            tools.contains(marker),
            "fixture postcondition marker missing"
        );
        let replacement = format!(
            "          \"error_code\": \"VerifyFailed\"\n        }},\n        {postcondition}\n      ]"
        );
        std::fs::write(path, tools.replacen(marker, &replacement, 1)).expect("write tools");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn test_adapter_validate_accepts_valid_fixture_without_writes() {
    let fixture = Fixture::create("valid");
    let before =
        std::fs::read_to_string(fixture.adapter.join("tools/tools.json")).expect("read tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 0, "{output}");
    assert!(output.contains("verdict: PASSED"), "{output}");
    assert!(output.contains("scanned_files=7"), "{output}");
    let after =
        std::fs::read_to_string(fixture.adapter.join("tools/tools.json")).expect("read tools");
    assert_eq!(before, after, "validator must not modify files");
}

#[test]
fn test_adapter_validate_rejects_bundle_path_traversal() {
    let fixture = Fixture::create("traversal");
    let manifest = std::fs::read_to_string(fixture.adapter.join("adapter.toml"))
        .expect("read manifest")
        .replace("selectors/targets.json", "../outside.json");
    std::fs::write(fixture.adapter.join("adapter.toml"), manifest).expect("write manifest");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("adapter/bundle-path"), "{output}");
}

#[test]
fn test_adapter_validate_reports_bundle_read_failure() {
    let fixture = Fixture::create("read-failure");
    let manifest = std::fs::read_to_string(fixture.adapter.join("adapter.toml"))
        .expect("read manifest")
        .replace("tools/tools.json", "tools");
    std::fs::write(fixture.adapter.join("adapter.toml"), manifest).expect("write manifest");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("adapter/bundle-read"), "{output}");
}

#[test]
fn test_adapter_validate_rejects_projection_byte_drift() {
    let fixture = Fixture::create("projection");
    std::fs::write(fixture.adapter.join("memory/app_map.v1.json"), "{}\n")
        .expect("write bad projection");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("adapter/projection-byte-drift"), "{output}");
}

#[test]
fn test_adapter_validate_rejects_text_selector_as_primary() {
    let fixture = Fixture::create("selector");
    let path = fixture.adapter.join("selectors/targets.json");
    let selectors = std::fs::read_to_string(&path)
        .expect("read selectors")
        .replace("\"kind\": \"automation_id\"", "\"kind\": \"exact_name\"")
        .replace(
            "\"value\": {\"text\": \"MainWindow\"}",
            "\"value\": {\"text\": \"Main window\"}",
        );
    std::fs::write(path, selectors).expect("write selectors");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("text/path selectors must be locale fallback"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_rejects_unknown_capability() {
    let fixture = Fixture::create("capability");
    let path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&path)
        .expect("read tools")
        .replace("\"target.list\"", "\"unknown.capability\"");
    std::fs::write(path, tools).expect("write tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("unknown capability"), "{output}");
}

#[test]
fn test_adapter_validate_rejects_invalid_tool_schema_metadata() {
    let fixture = Fixture::create("tool-metadata");
    let path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&path)
        .expect("read tools")
        .replace("\"name\": \"test.window.list\"", "\"name\": \"Bad\"")
        .replace("\"risk_level\": \"low\"", "\"risk_level\": \"urgent\"")
        .replace("\"tags\": [\"test\"]", "\"tags\": [\"Bad Tag\"]");
    std::fs::write(path, tools).expect("write tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("tool name invalid"), "{output}");
    assert!(output.contains("risk_level must be"), "{output}");
    assert!(output.contains("tag must match"), "{output}");
}

#[test]
fn test_adapter_validate_rejects_tool_envelope_without_schema() {
    let fixture = Fixture::create("missing-schema");
    let path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&path)
        .expect("read tools")
        .replace("\"schema\": {", "\"schema_missing\": {");
    std::fs::write(path, tools).expect("write tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("schema missing"), "{output}");
}

#[test]
fn test_adapter_validate_rejects_postcondition_type_mismatch() {
    let fixture = Fixture::create("postcondition-type");
    let path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&path).expect("read tools").replace(
        "\"fingerprint_scope\": \"whole_window\"",
        "\"fingerprint_scope\": 3",
    );
    std::fs::write(path, tools).expect("write tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("does not satisfy catalog type"), "{output}");
}

#[test]
fn test_adapter_validate_executes_fingerprint_constraint() {
    let fixture = Fixture::create("postcondition-fingerprint");
    fixture.append_postcondition(
        r#"{
          "kind": "state_assert",
          "field": "fingerprint",
          "op": "equals",
          "value": "sha256:not-a-digest",
          "description": "Fingerprint must match.",
          "error_code": "VerifyFailed"
        }"#,
    );
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("violates catalog constraint type `fingerprint`"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_executes_title_value_type_constraint() {
    let fixture = Fixture::create("postcondition-title-type");
    fixture.append_postcondition(
        r#"{
          "kind": "state_assert",
          "field": "title",
          "op": "equals",
          "value": 7,
          "description": "Title must match.",
          "error_code": "VerifyFailed"
        }"#,
    );
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("violates catalog constraint type `string`"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_executes_if_field_equals_constraint() {
    let fixture = Fixture::create("postcondition-field-op");
    fixture.append_postcondition(
        r#"{
          "kind": "state_assert",
          "field": "fingerprint",
          "op": "contains",
          "value": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
          "description": "Fingerprint must match.",
          "error_code": "VerifyFailed"
        }"#,
    );
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("violates catalog constraint `field=fingerprint`"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_executes_numeric_range_constraint() {
    let fixture = Fixture::create("postcondition-range");
    fixture.append_postcondition(
        r#"{
          "kind": "value_in_range",
          "name": "amount",
          "min": 2,
          "max": 1,
          "description": "Amount must stay in range.",
          "error_code": "VerifyFailed"
        }"#,
    );
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("violates catalog constraint `min <= max`"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_compares_large_integers_without_precision_loss() {
    let fixture = Fixture::create("postcondition-large-integer-range");
    fixture.append_postcondition(
        r#"{
          "kind": "value_in_range",
          "name": "amount",
          "min": 9007199254740993,
          "max": 9007199254740992,
          "description": "Amount must stay in range.",
          "error_code": "VerifyFailed"
        }"#,
    );
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("violates catalog constraint `min <= max`"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_compares_large_integer_and_float_without_precision_loss() {
    let fixture = Fixture::create("postcondition-mixed-number-range");
    fixture.append_postcondition(
        r#"{
          "kind": "value_in_range",
          "name": "amount",
          "min": 9007199254740993,
          "max": 9007199254740992.0,
          "description": "Amount must stay in range.",
          "error_code": "VerifyFailed"
        }"#,
    );
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("violates catalog constraint `min <= max`"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_rejects_unknown_catalog_constraint_tag() {
    let fixture = Fixture::create("postcondition-unknown-constraint");
    let catalog_path = fixture.root.join(adapter_catalog::CATALOG_PATH);
    let catalog = std::fs::read_to_string(&catalog_path)
        .expect("read catalog")
        .replacen(
            "\"kind\": \"field_value_type\"",
            "\"kind\": \"unknown_constraint\"",
            1,
        );
    std::fs::write(catalog_path, catalog).expect("write catalog");
    fixture.append_postcondition(
        r#"{
          "kind": "state_assert",
          "field": "fingerprint",
          "op": "equals",
          "value": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
          "description": "Fingerprint must match.",
          "error_code": "VerifyFailed"
        }"#,
    );
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("unknown catalog constraint kind `unknown_constraint`"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_rejects_malformed_if_field_equals_metadata() {
    let fixture = Fixture::create("postcondition-malformed-if-equals");
    let catalog_path = fixture.root.join(adapter_catalog::CATALOG_PATH);
    let catalog = std::fs::read_to_string(&catalog_path)
        .expect("read catalog")
        .replacen("\"equals\": \"fingerprint\"", "\"equals\": \"\"", 1);
    std::fs::write(catalog_path, catalog).expect("write catalog");
    fixture.append_postcondition(
        r#"{
          "kind": "state_assert",
          "field": "title",
          "op": "equals",
          "value": "Window title",
          "description": "Title must match.",
          "error_code": "VerifyFailed"
        }"#,
    );
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("catalog constraint `if_field_equals` missing equals"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_rejects_write_without_rollback_coverage() {
    let fixture = Fixture::create("rollback");
    let path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&path)
        .expect("read tools")
        .replace("\"effect\": \"read\"", "\"effect\": \"write\"")
        .replace(
            "\"reversibility\": \"none_readonly\"",
            "\"reversibility\": \"l2_compensation\"",
        )
        .replace(
            "\"kind\": \"state_unchanged\"",
            "\"kind\": \"state_changed\"",
        )
        .replace(
            "\"fingerprint_scope\": \"whole_window\"",
            "\"within_ms\": 1000",
        );
    std::fs::write(path, tools).expect("write tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("must have exactly one coverage"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_rejects_empty_rollback_anchor_union() {
    let fixture = Fixture::create("empty-anchor");
    let tools_path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&tools_path)
        .expect("read tools")
        .replace("\"effect\": \"read\"", "\"effect\": \"write\"")
        .replace(
            "\"reversibility\": \"none_readonly\"",
            "\"reversibility\": \"l2_compensation\"",
        )
        .replace(
            "\"kind\": \"state_unchanged\"",
            "\"kind\": \"state_changed\"",
        )
        .replace(
            "\"fingerprint_scope\": \"whole_window\"",
            "\"within_ms\": 1000",
        );
    std::fs::write(tools_path, tools).expect("write tools");
    let rollback = r#"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "recipes": [
    {
      "tool": "test.window.list",
      "coverage": "recipe",
      "anchor_strategy": [
        {
          "level": "l2",
          "mechanism": "compensating_action",
          "capture": {},
          "restore": {},
          "post_verify": {}
        }
      ],
      "on_rollback_failure": "publish_incident_and_require_human"
    }
  ]
}
"#;
    std::fs::write(fixture.adapter.join("rollback/recipes.json"), rollback)
        .expect("write rollback");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("capture/kind"), "{output}");
    assert!(output.contains("restore/kind"), "{output}");
    assert!(output.contains("post_verify/kind"), "{output}");
}

#[test]
fn test_adapter_validate_accepts_close_created_tab_builtin() {
    let fixture = Fixture::create("close-created-tab-valid");
    fixture.write_created_tab_bundle();
    let (code, output) = fixture.validate();
    assert_eq!(code, 0, "{output}");
    assert!(output.contains("verdict: PASSED"), "{output}");
}
#[test]
fn test_adapter_validate_rejects_close_created_non_tab_target() {
    let fixture = Fixture::create("close-created-window");
    fixture.write_created_tab_bundle();
    let path = fixture.adapter.join("rollback/recipes.json");
    let rollback = std::fs::read_to_string(&path)
        .expect("read rollback")
        .replace("\"target_kind\": \"tab\"", "\"target_kind\": \"window\"");
    std::fs::write(path, rollback).expect("write rollback");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("target_kind must be tab"), "{output}");
}
#[test]
fn test_adapter_validate_rejects_close_created_non_empty_tab() {
    let fixture = Fixture::create("close-created-non-empty");
    fixture.write_created_tab_bundle();
    let path = fixture.adapter.join("rollback/recipes.json");
    let rollback = std::fs::read_to_string(&path)
        .expect("read rollback")
        .replace("\"require_empty\": true", "\"require_empty\": false");
    std::fs::write(path, rollback).expect("write rollback");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("require_empty must be true"), "{output}");
}
#[test]
fn test_adapter_validate_rejects_close_created_without_tab_new_tags() {
    let fixture = Fixture::create("close-created-tags");
    fixture.write_created_tab_bundle();
    let path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&path)
        .expect("read tools")
        .replace("\"tags\": [\"new\", \"tab\"]", "\"tags\": [\"tab\"]");
    std::fs::write(path, tools).expect("write tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("must carry tab and new tags"), "{output}");
}
#[test]
fn test_adapter_validate_rejects_close_created_wrong_reversibility() {
    let fixture = Fixture::create("close-created-reversibility");
    fixture.write_created_tab_bundle();
    let path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&path).expect("read tools").replace(
        "\"reversibility\": \"l2_compensation\"",
        "\"reversibility\": \"l1_snapshot\"",
    );
    std::fs::write(path, tools).expect("write tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("target tool must be write + l2_compensation"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_rejects_close_created_without_fingerprint_observation() {
    let fixture = Fixture::create("close-created-fingerprint");
    fixture.write_created_tab_bundle();
    let path = fixture.adapter.join("rollback/recipes.json");
    let rollback = std::fs::read_to_string(&path)
        .expect("read rollback")
        .replace("[\"whole_window_fingerprint\"]", "[\"tab_count\"]")
        .replace(
            "\"observation\": \"whole_window_fingerprint\"",
            "\"observation\": \"tab_count\"",
        );
    std::fs::write(path, rollback).expect("write rollback");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("requires whole_window_fingerprint"),
        "{output}"
    );
}

#[test]
fn test_adapter_validate_rejects_close_created_unknown_restore_field() {
    let fixture = Fixture::create("close-created-extra-field");
    fixture.write_created_tab_bundle();
    let path = fixture.adapter.join("rollback/recipes.json");
    let rollback = std::fs::read_to_string(&path)
        .expect("read rollback")
        .replace(
            "\"require_empty\": true",
            "\"require_empty\": true,\n            \"tool\": \"test.tab.new\"",
        );
    std::fs::write(path, rollback).expect("write rollback");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("unknown field `tool`"), "{output}");
}

#[test]
fn test_adapter_validate_rejects_text_only_interrupt_and_duplicate_priority() {
    let fixture = Fixture::create("interrupt");
    let value = r##"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "default_action": "cancel",
  "interrupts": [
    {
      "id": "text-only",
      "priority": 10,
      "match": {"title_candidates": [{"value": "Save", "locale_dependent": true}]},
      "action": {"kind": "cancel"},
      "classification": "transient_popup",
      "requires_human": false
    },
    {
      "id": "duplicate",
      "priority": 10,
      "match": {"class_name": "#32770"},
      "action": {"kind": "cancel"},
      "classification": "transient_popup",
      "requires_human": false
    }
  ]
}
"##;
    std::fs::write(fixture.adapter.join("interrupts/interrupts.json"), value)
        .expect("write interrupts");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(
        output.contains("text candidates cannot be sole match"),
        "{output}"
    );
    assert!(output.contains("duplicate priority"), "{output}");
}

#[test]
fn test_adapter_validate_rejects_incomplete_selector_policy() {
    let fixture = Fixture::create("selector-policy");
    let path = fixture.adapter.join("selectors/targets.json");
    let selectors = std::fs::read_to_string(&path)
        .expect("read selectors")
        .replace("\"retry_after_ms\": []", "\"retry_after_ms\": \"bad\"");
    std::fs::write(path, selectors).expect("write selectors");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("retry_after_ms"), "{output}");
}

#[test]
fn test_adapter_validate_rejects_interrupt_human_matrix_mismatch() {
    let fixture = Fixture::create("interrupt-human");
    let value = r##"{
  "schema_version": 1,
  "app_id": "com.example.test",
  "default_action": "cancel",
  "interrupts": [
    {
      "id": "ask",
      "priority": 10,
      "match": {"class_name": "#32770"},
      "action": {"kind": "ask_human"},
      "classification": "transient_popup",
      "requires_human": false,
      "resume_after": "user_decision"
    }
  ]
}
"##;
    std::fs::write(fixture.adapter.join("interrupts/interrupts.json"), value)
        .expect("write interrupts");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("ask_human requires"), "{output}");
    assert!(output.contains("must omit resume_after"), "{output}");
}

#[test]
fn test_adapter_validate_rejects_projection_model_drift_and_float_versions() {
    let fixture = Fixture::create("projection-model-float");
    let app_map_path = fixture.adapter.join("app_map.json");
    let app_map = std::fs::read_to_string(&app_map_path)
        .expect("read app map")
        .replace(
            "\"model\": \"core_app_map_v1\"",
            "\"model\": \"wrong_model\"",
        );
    std::fs::write(app_map_path, app_map).expect("write app map");
    let tools_path = fixture.adapter.join("tools/tools.json");
    let tools = std::fs::read_to_string(&tools_path)
        .expect("read tools")
        .replace("\"schema_version\": 1", "\"schema_version\": 1.0");
    std::fs::write(tools_path, tools).expect("write tools");
    let (code, output) = fixture.validate();
    assert_eq!(code, 1);
    assert!(output.contains("must be core_app_map_v1"), "{output}");
    assert!(output.contains("document version must be 1"), "{output}");
}

//! 注册期「形状拒绝」契约：`collect_unenforceable_constructs` 对**形状不合法**的受支持关键字
//! 必须 fail-closed，并给出可读原因 + schema 文档内 JSON pointer。
//!
//! 为什么是集成测试：这里断言的全是**公开契约**（`collect_unenforceable_constructs` 的返回值），
//! 不过生产实现细节；目标是让 README 承诺的「形状不对同样算无法强制」这条 fail-closed 判据
//! 每个分支都有可回归证据（铁律 1「无静默失败」）。
//!
//! 只调用公开 API，不改生产逻辑。测试 lint 例外仅由本文件顶部一行显式放开（AGENTS.md §5.3）。

#![allow(clippy::panic)]

use assistant_tool_bus::collect_unenforceable_constructs;
use serde_json::{Map, Value, json};

/// 包一个只含 `keyword: value` 的 schema（本文件的用例几乎都是这个形状）。
fn schema_with(keyword: &str, value: Value) -> Value {
    let mut map = Map::new();
    map.insert(keyword.to_owned(), value);
    Value::Object(map)
}

/// 取唯一一条问题（形状不合法的关键字应当只产生一条、且可读）。
fn only_problem(schema: &Value) -> String {
    let problems = collect_unenforceable_constructs(schema);
    assert_eq!(problems.len(), 1, "{schema}：{problems:?}");
    problems.first().cloned().unwrap_or_default()
}

/// 根不是对象、也不是布尔 schema → 视为「读不懂」，拒绝并指到 `<root>`。
///
/// draft-07 的 `true` / `false` 是合法布尔 schema（`false` = 谁都不过），必须放行。
#[test]
fn test_non_object_schema_is_rejected_but_booleans_pass() {
    for schema in [json!("x"), json!(7), json!([1, 2]), Value::Null] {
        let message = only_problem(&schema);
        assert!(message.starts_with("<root>:"), "{schema}：{message}");
        assert!(
            message.contains("expected a JSON Schema object"),
            "{schema}：{message}"
        );
    }
    for schema in [json!(true), json!(false)] {
        assert!(
            collect_unenforceable_constructs(&schema).is_empty(),
            "{schema} 是合法布尔 schema，应被接受"
        );
    }
}

/// `type` / `enum` / `required` 的形状判据：类型名非法、`enum` 非数组、`required` 非字符串数组
/// 都要拒绝。
#[test]
fn test_malformed_type_enum_and_required_are_rejected() {
    let cases = [
        ("type", json!(7), "malformed `type`"),
        ("type", json!([]), "malformed `type`"),
        ("type", json!(["string", 7]), "malformed `type`"),
        ("enum", json!(7), "malformed `enum`"),
        ("required", json!("a"), "malformed `required`"),
        ("required", json!(["a", 7]), "malformed `required`"),
    ];
    for (keyword, value, fragment) in cases {
        let message = only_problem(&schema_with(keyword, value));
        assert!(message.contains(fragment), "{keyword}：{message}");
    }
}

/// 非负整数边界（长度 / 个数）与数值边界（含 draft-04 布尔写法）都要按 draft-07 拒绝。
#[test]
fn test_malformed_numeric_bounds_are_rejected() {
    for keyword in [
        "minProperties",
        "maxProperties",
        "minItems",
        "maxItems",
        "minLength",
        "maxLength",
    ] {
        let message = only_problem(&schema_with(keyword, json!(-1)));
        assert!(
            message.contains(&format!(
                "malformed `{keyword}` (expected a non-negative integer)"
            )),
            "{keyword}：{message}"
        );
    }
    for keyword in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
        let message = only_problem(&schema_with(keyword, json!(true)));
        assert!(
            message.contains(&format!("malformed `{keyword}`")),
            "{keyword}：{message}"
        );
        assert!(
            message.contains("draft-07 expects a number"),
            "{keyword}：{message}"
        );
    }
}

/// 含子 schema 的关键字：`properties` 非对象、`additionalProperties` 非布尔非 schema、
/// `items` 元组形式、组合器非数组、组合器空数组，全部拒绝。
#[test]
fn test_malformed_sub_schema_keywords_are_rejected() {
    let cases = [
        ("properties", json!(7), "malformed `properties`"),
        (
            "additionalProperties",
            json!(7),
            "malformed `additionalProperties`",
        ),
        ("items", json!(7), "malformed `items`"),
        ("items", json!([{}]), "tuple form is not supported"),
        ("allOf", json!({}), "malformed `allOf`"),
        ("anyOf", json!(7), "malformed `anyOf`"),
    ];
    for (keyword, value, fragment) in cases {
        let message = only_problem(&schema_with(keyword, value));
        assert!(message.contains(fragment), "{keyword}：{message}");
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        let message = only_problem(&schema_with(keyword, json!([])));
        assert!(
            message.contains(&format!("`{keyword}` must not be empty")),
            "{keyword}：{message}"
        );
    }
}

/// 形状错误发生在嵌套子 schema 时，pointer 必须指到 schema 文档里的具体位置。
#[test]
fn test_malformed_shape_reports_nested_pointer() {
    let message = only_problem(&json!({"properties": {"x": {"items": [1]}}}));
    assert!(message.starts_with("/properties/x:"), "{message}");
    assert!(message.contains("malformed `items`"), "{message}");
}

/// 合法形状必须放行：布尔 / 对象子 schema、`const` 任意取值都不得被误拒。
#[test]
fn test_valid_boolean_and_object_sub_schemas_pass() {
    for schema in [
        json!({"additionalProperties": true}),
        json!({"additionalProperties": {}}),
        json!({"items": {}}),
        json!({"items": true}),
        json!({"not": {}}),
        json!({"properties": {}}),
        json!({"const": {"any": "value"}}),
    ] {
        assert!(
            collect_unenforceable_constructs(&schema).is_empty(),
            "{schema} 是合法形状，应被接受"
        );
    }
}

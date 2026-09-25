//! draft-07 关键字判据的**整齐性**与**可读性**（TASK-204 交付物 1~3）。
//!
//! 为什么是集成测试而不是 `#[cfg(test)]` 模块：这里断言的全是**公开契约** —— 五张表的成员、
//! 拒绝消息的标签与 JSON pointer、`$schema` 的方言判据。放在 `tests/` 才能保证 `pub use` 真的
//! 可达，即「Adapter 作者读得到的东西 = 本 crate 承诺的东西」。
//!
//! `panic` 的例外由本文件顶部一行显式放开（AGENTS.md §5.3）；其余禁用项（`unwrap` /
//! `expect` / `indexing_slicing`）**不放开** —— 用例一律走 `.first()` / `.get()`。

#![allow(clippy::panic)]

use assistant_tool_bus::{
    ANNOTATION_KEYWORDS, NON_DRAFT07_KEYWORDS, REJECTED_FOR_NOW_KEYWORDS,
    REJECTED_FOREVER_KEYWORDS, SUPPORTED_KEYWORDS, collect_unenforceable_constructs,
    validate_arguments,
};
use serde_json::{Map, Value, json};

/// 包一个只含 `keyword: value` 的 schema（本文件里 90% 的用例都是这个形状）。
fn schema_with(keyword: &str, value: Value) -> Value {
    let mut map = Map::new();
    map.insert(keyword.to_owned(), value);
    Value::Object(map)
}

/// 取唯一一条问题（本文件里除「条数上限」以外的用例都只期望一条）。
fn only_problem(schema: &Value) -> String {
    let problems = collect_unenforceable_constructs(schema);
    assert_eq!(problems.len(), 1, "{schema}：{problems:?}");
    problems.first().cloned().unwrap_or_default()
}

/// 五张表必须**两两不相交**且各自内部不重复。
///
/// 为什么这是硬不变量：一个关键字若同时落在两张表里，`classify_keyword` 的固定顺序就会**悄悄**
/// 决定它算哪一类 —— 于是「文档说 A、报错说 B、行为是 C」三者可能互相矛盾，而这正是本卡要
/// 消灭的形态（TASK-020 §9 关注点 3「最大设计负债」）。
#[test]
fn test_keyword_tables_are_pairwise_disjoint_and_unique() {
    let tables: [(&str, Vec<&str>); 5] = [
        ("SUPPORTED_KEYWORDS", SUPPORTED_KEYWORDS.to_vec()),
        ("ANNOTATION_KEYWORDS", ANNOTATION_KEYWORDS.to_vec()),
        (
            "REJECTED_FOREVER_KEYWORDS",
            REJECTED_FOREVER_KEYWORDS
                .iter()
                .map(|(name, _)| *name)
                .collect(),
        ),
        (
            "REJECTED_FOR_NOW_KEYWORDS",
            REJECTED_FOR_NOW_KEYWORDS
                .iter()
                .map(|(name, _)| *name)
                .collect(),
        ),
        (
            "NON_DRAFT07_KEYWORDS",
            NON_DRAFT07_KEYWORDS.iter().map(|(name, _)| *name).collect(),
        ),
    ];

    for (table, keywords) in &tables {
        let mut sorted = keywords.clone();
        sorted.sort_unstable();
        let total = sorted.len();
        sorted.dedup();
        assert_eq!(total, sorted.len(), "{table} 表内有重复项");
    }

    let mut seen: Vec<(String, String)> = Vec::new();
    for (table, keywords) in &tables {
        for keyword in keywords {
            if let Some((owner, _)) = seen.iter().find(|(_, name)| name == keyword) {
                panic!("`{keyword}` 同时在 {owner} 与 {table} 里");
            }
            seen.push(((*table).to_owned(), (*keyword).to_owned()));
        }
    }
    assert_eq!(
        seen.len(),
        21 + 9 + 12 + 4 + 15,
        "五张表的规模变了：请同步文档"
    );
}

/// 白名单里的每个关键字都必须**真的被接受**；形状表反过来也不许多出白名单之外的名字。
#[test]
fn test_every_supported_keyword_accepts_its_valid_shape() {
    let valid_shapes: &[(&str, Value)] = &[
        ("type", json!("object")),
        ("enum", json!([1, 2])),
        ("const", json!(1)),
        ("properties", json!({})),
        ("required", json!(["a"])),
        ("additionalProperties", json!(false)),
        ("minProperties", json!(0)),
        ("maxProperties", json!(1)),
        ("items", json!({})),
        ("minItems", json!(0)),
        ("maxItems", json!(1)),
        ("minLength", json!(0)),
        ("maxLength", json!(1)),
        ("minimum", json!(0)),
        ("maximum", json!(1)),
        ("exclusiveMinimum", json!(0)),
        ("exclusiveMaximum", json!(1)),
        ("allOf", json!([{}])),
        ("anyOf", json!([{}])),
        ("oneOf", json!([{}])),
        ("not", json!({})),
    ];

    let mut names: Vec<&str> = valid_shapes.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    let mut supported = SUPPORTED_KEYWORDS.to_vec();
    supported.sort_unstable();
    assert_eq!(names, supported, "形状表必须与 SUPPORTED_KEYWORDS 逐一对应");

    for (keyword, value) in valid_shapes {
        let problems = collect_unenforceable_constructs(&schema_with(keyword, value.clone()));
        assert!(
            problems.is_empty(),
            "`{keyword}` 的合法形状被拒：{problems:?}"
        );
    }
}

/// 三张拒绝表的每一条都必须：带对应**标签**、带**理由**、带根 JSON pointer。
#[test]
fn test_rejected_tables_produce_labelled_messages_with_reason() {
    for (keyword, reason) in REJECTED_FOREVER_KEYWORDS {
        let message = only_problem(&schema_with(keyword, json!({})));
        assert!(message.starts_with("<root>:"), "`{keyword}`：{message}");
        assert!(
            message.contains("is permanently unsupported"),
            "`{keyword}` 缺少永久放弃标签：{message}"
        );
        assert!(message.contains(reason), "`{keyword}` 缺少理由：{message}");
    }

    for (keyword, reason) in REJECTED_FOR_NOW_KEYWORDS {
        let message = only_problem(&schema_with(keyword, json!({})));
        assert!(message.starts_with("<root>:"), "`{keyword}`：{message}");
        assert!(
            message.contains("is not supported yet"),
            "`{keyword}` 缺少暂未实现标签：{message}"
        );
        assert!(message.contains(reason), "`{keyword}` 缺少理由：{message}");
    }

    for (keyword, dialect) in NON_DRAFT07_KEYWORDS {
        let message = only_problem(&schema_with(keyword, json!({})));
        let expected =
            format!("is a JSON Schema {dialect} keyword, but this crate implements draft-07 only");
        assert!(message.contains(&expected), "`{keyword}`：{message}");
    }
}

/// 拒绝消息必须能指到**具体位置**：嵌套进 `properties` 时是 `/properties/<name>` 这样的
/// **schema 文档内** JSON pointer（不是实例路径，也不是笼统的根）。
#[test]
fn test_nested_rejection_reports_json_pointer() {
    let schema = json!({"properties": {"name": {"pattern": "^[a-z]+$"}}});
    let message = only_problem(&schema);
    assert!(
        message.starts_with("/properties/name:"),
        "pointer 必须是 schema 文档内的 JSON pointer：{message}"
    );
}

/// `$schema` 不是注解：draft-07（或缺省）放行，别的方言必须拒绝并把方言写进消息。
#[test]
fn test_schema_dialect_is_checked_instead_of_ignored() {
    let accepted = [
        json!({}),
        json!({"$schema": "http://json-schema.org/draft-07/schema#"}),
        json!({"$schema": "https://json-schema.org/draft-07/schema"}),
        json!({"$schema": "  https://json-schema.org/draft-07/schema#  "}),
    ];
    for schema in accepted {
        let problems = collect_unenforceable_constructs(&schema);
        assert!(problems.is_empty(), "{schema} 应被接受：{problems:?}");
    }

    let rejected = [
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema"}),
        json!({"$schema": "http://json-schema.org/draft-04/schema#"}),
        json!({"$schema": 7}),
    ];
    for schema in rejected {
        let message = only_problem(&schema);
        assert!(message.contains("`$schema`"), "{schema}：{message}");
    }

    let message = only_problem(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema"
    }));
    assert!(message.contains("2020-12"), "方言要出现在消息里：{message}");
}

/// 三张表之外的任何关键字 → 拒绝，且提示这是「不认识」而不是「永久放弃」。
#[test]
fn test_unknown_keyword_gets_a_typo_hint() {
    let message = only_problem(&json!({"propterties": {}}));
    assert!(
        message.contains("unknown keyword `propterties`"),
        "{message}"
    );
    assert!(message.contains("check the spelling"), "{message}");
}

/// 注解表里的关键字：忽略，**但不是**「支持」。
#[test]
fn test_annotations_are_ignored_without_becoming_assertions() {
    let schema = json!({
        "title": "t",
        "description": "d",
        "default": 1,
        "examples": [1],
        "$comment": "c",
        "deprecated": true,
        "readOnly": true,
        "writeOnly": false,
        "$id": "https://example.invalid/tool",
        "type": "object"
    });
    let problems = collect_unenforceable_constructs(&schema);
    assert!(problems.is_empty(), "{problems:?}");
}

/// 深度上限仍在（本卡没有放松对抗性输入的护栏）。
#[test]
fn test_schema_nesting_depth_is_still_capped() {
    let mut schema = json!({});
    for _ in 0..70 {
        schema = json!({"items": schema});
    }
    let message = only_problem(&schema);
    assert!(message.contains("nests deeper than 64 levels"), "{message}");
}

/// 实例校验的「最多 8 条 + 1 行汇总」也仍在。
#[test]
fn test_instance_violations_are_capped_with_a_summary_line() {
    let schema = json!({
        "type": "object",
        "required": ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]
    });
    let arguments = Map::new();
    let violations = validate_arguments(&schema, &arguments);
    assert_eq!(violations.len(), 9, "{violations:?}");
    assert!(
        violations
            .last()
            .is_some_and(|line| line.contains("and 2 more violation(s)")),
        "{violations:?}"
    );
}

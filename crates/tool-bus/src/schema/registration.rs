//! Registration-time schema checks: reject constructs this crate cannot enforce.
//!
//! This module owns the keyword tables, dialect validation, shape checks, and
//! recursive schema walk. Runtime instance validation lives in `instance.rs`.

use serde_json::Value;

use super::shared::{MAX_NESTING_DEPTH, at, child_pointer, child_pointer_index};

/// 本 crate **能强制**的关键字白名单（出现即校验语义 + 检查形状）。
pub const SUPPORTED_KEYWORDS: &[&str] = &[
    "type",
    "enum",
    "const",
    "properties",
    "required",
    "additionalProperties",
    "minProperties",
    "maxProperties",
    "items",
    "minItems",
    "maxItems",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
];

/// **永久放弃**的 draft-07 关键字 → 一句理由 +（能给的）替代方案。
///
/// 「永久」= 已裁决的设计决定，**不要再提案重开**（理由与替代见
/// `docs/memory/rejected.md`）。判据一律是「读了但不强制 = 静默失败」（铁律 1）。
pub const REJECTED_FOREVER_KEYWORDS: &[(&str, &str)] = &[
    (
        "$ref",
        "cross-document resolution + recursion + SSRF surface; inline the sub-schema",
    ),
    (
        "definitions",
        "only meaningful together with `$ref`, which this crate never resolves",
    ),
    (
        "pattern",
        "regex = new dependency + ReDoS surface; use `enum`, or validate in the handler",
    ),
    (
        "patternProperties",
        "same regex objection as `pattern`; use a closed set of `properties`",
    ),
    (
        "format",
        "implementation-defined in draft-07 so it cannot be enforced; use `enum`",
    ),
    (
        "if",
        "non-enumerable for the model; use `oneOf` with `const` discriminators",
    ),
    (
        "then",
        "only meaningful together with `if`; use `oneOf` with `const` discriminators",
    ),
    (
        "else",
        "only meaningful together with `if`; use `oneOf` with `const` discriminators",
    ),
    (
        "dependencies",
        "conditionals in disguise; use `oneOf` with `const` discriminators",
    ),
    (
        "additionalItems",
        "needs tuple-form `items` (rejected): use named object fields instead",
    ),
    (
        "contentEncoding",
        "an annotation in draft-07 read as an assertion; validate in the handler",
    ),
    (
        "contentMediaType",
        "an annotation in draft-07 read as an assertion; validate in the handler",
    ),
];

/// draft-07 里**存在**、但本 crate **尚未实现**的关键字 → 一句理由（= 缺什么）。
///
/// 与 [`REJECTED_FOREVER_KEYWORDS`] 的区别：这些是**缺口**（可以有卡），不是**决定**。
pub const REJECTED_FOR_NOW_KEYWORDS: &[(&str, &str)] = &[
    (
        "multipleOf",
        "pure numeric assertion, no dependency needed; open a card if needed",
    ),
    (
        "uniqueItems",
        "pure array assertion (value comparison); open a card if an adapter needs it",
    ),
    (
        "contains",
        "array assertion over a sub-schema; open a card if an adapter needs it",
    ),
    (
        "propertyNames",
        "applies a sub-schema to each key, no regex needed; open a card if needed",
    ),
];

/// 属于**其它草案**的关键字 → 第二元素 = 引入它的方言年份。
///
/// 本 crate 只实现 draft-07：这些关键字一旦出现，说明作者拿的是更新方言的文档 —— 必须
/// 抬出来让他降方言，而不是按 draft-07 的相似语义**猜**（`items` 在 2020-12 里才是单
/// schema 形式，元组形式已改名为 `prefixItems`）。
pub const NON_DRAFT07_KEYWORDS: &[(&str, &str)] = &[
    ("$defs", "2020-12"),
    ("$anchor", "2019-09"),
    ("$recursiveRef", "2019-09"),
    ("$recursiveAnchor", "2019-09"),
    ("$dynamicRef", "2020-12"),
    ("$dynamicAnchor", "2020-12"),
    ("$vocabulary", "2019-09"),
    ("unevaluatedProperties", "2019-09"),
    ("unevaluatedItems", "2019-09"),
    ("dependentSchemas", "2019-09"),
    ("dependentRequired", "2019-09"),
    ("minContains", "2019-09"),
    ("maxContains", "2019-09"),
    ("prefixItems", "2020-12"),
    ("contentSchema", "2019-09"),
];

/// 声明解释方言的关键字（唯一一个「既不是断言、也不是注解」的 draft-07 关键字）。
const DIALECT_KEYWORD: &str = "$schema";

/// `$schema` 的合法取值（比较前去掉结尾 `#`；http / https 两种历史写法都接受）。
const DRAFT07_SCHEMA_URIS: &[&str] = &[
    "http://json-schema.org/draft-07/schema",
    "https://json-schema.org/draft-07/schema",
];

/// 纯注解关键字：不影响校验结果，忽略（**不**拒绝）。
///
/// 这里的每一条都**不改变校验结果**，所以「读了不强制」不算静默失败；`$schema` 刻意
/// **不在**本表里（它改变解释方言 → 由 [`check_declared_dialect`] 校验）。
pub const ANNOTATION_KEYWORDS: &[&str] = &[
    "title",
    "description",
    "default",
    "examples",
    "$comment",
    "deprecated",
    "readOnly",
    "writeOnly",
    "$id",
];

/// 允许出现的 JSON 类型名（unknown 名字 = 无法强制 → 拒绝）。
const JSON_TYPES: &[&str] = &[
    "object", "array", "string", "number", "integer", "boolean", "null",
];

/// 关键字的归类结果 —— [`classify_keyword`] 的唯一输出，驱动 [`walk_schema`] 的分支。
enum KeywordVerdict {
    /// 白名单：[`SUPPORTED_KEYWORDS`] 成员，出现即校验语义 + 检查形状。
    Supported,
    /// 纯注解：[`ANNOTATION_KEYWORDS`] 成员，忽略。
    Annotation,
    /// `$schema`：走方言校验（不是注解）。
    Dialect,
    /// draft-07 关键字，本项目**永久**不支持（附理由 + 替代）。
    NeverSupported(&'static str),
    /// draft-07 关键字，本项目**尚未**实现（附理由）。
    NotYetSupported(&'static str),
    /// 属于其它草案的关键字（附方言年份）。
    OtherDialect(&'static str),
    /// 完全不认识（拼写错误 / 自定义关键字）。
    Unknown,
}

/// 按「支持 → 注解 → 方言 → 永久拒绝 → 暂未实现 → 其它方言 → 未知」的**固定顺序**归类。
///
/// 顺序即优先级；同时**五张表必须两两不相交**（否则同一关键字会出现两种说法），这条
/// 不变量由 `tests/schema_keywords.rs` 的
/// `test_keyword_tables_are_pairwise_disjoint_and_unique` 强制。
fn classify_keyword(keyword: &str) -> KeywordVerdict {
    if SUPPORTED_KEYWORDS.contains(&keyword) {
        return KeywordVerdict::Supported;
    }
    if ANNOTATION_KEYWORDS.contains(&keyword) {
        return KeywordVerdict::Annotation;
    }
    if keyword == DIALECT_KEYWORD {
        return KeywordVerdict::Dialect;
    }
    if let Some(&(_, reason)) = REJECTED_FOREVER_KEYWORDS
        .iter()
        .find(|&&(name, _)| name == keyword)
    {
        return KeywordVerdict::NeverSupported(reason);
    }
    if let Some(&(_, reason)) = REJECTED_FOR_NOW_KEYWORDS
        .iter()
        .find(|&&(name, _)| name == keyword)
    {
        return KeywordVerdict::NotYetSupported(reason);
    }
    if let Some(&(_, dialect)) = NON_DRAFT07_KEYWORDS
        .iter()
        .find(|&&(name, _)| name == keyword)
    {
        return KeywordVerdict::OtherDialect(dialect);
    }
    KeywordVerdict::Unknown
}

/// `$schema` 的方言校验：缺省 = 按 draft-07 处理；声明 draft-07 = 放行；声明别的方言 = 拒绝。
///
/// 为什么不能当注解：见本文件模块文档「`$schema` 不忽略，改做**方言校验**」一节。取值不是
/// 字符串同样算「无法强制」（我们读不懂这份文档按什么语义解释）。
fn check_declared_dialect(value: &Value, pointer: &str, problems: &mut Vec<String>) {
    let Value::String(uri) = value else {
        problems.push(format!(
            "{}: `$schema` must be a string (a URI naming the JSON Schema dialect)",
            at(pointer)
        ));
        return;
    };
    if DRAFT07_SCHEMA_URIS.contains(&uri.trim().trim_end_matches('#')) {
        return;
    }
    problems.push(format!(
        "{}: `$schema` declares `{uri}`, but this crate only enforces JSON Schema draft-07",
        at(pointer)
    ));
}

/// 收集 schema 里**本 crate 无法强制**的构造（不支持的关键字、或形状不合法的关键字）。
///
/// 返回空 Vec = 可以注册。返回的每条都是「`<JSON pointer>`: 原因」，顺序稳定
/// （`serde_json::Map` 默认按 key 有序，加上显式递归顺序）。
///
/// 幂等 / 无副作用：纯函数。
#[must_use]
pub fn collect_unenforceable_constructs(schema: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    walk_schema(schema, "", 0, &mut problems);
    problems
}

/// 递归遍历 schema 的**所有**子 schema 位置，收集无法强制的构造。
fn walk_schema(schema: &Value, pointer: &str, depth: usize, problems: &mut Vec<String>) {
    if depth > MAX_NESTING_DEPTH {
        problems.push(format!(
            "{}: schema nests deeper than {MAX_NESTING_DEPTH} levels",
            at(pointer)
        ));
        return;
    }
    let Value::Object(object) = schema else {
        // `true` / `false` 是 draft-07 的合法布尔 schema（`false` = 谁都不过）。
        if !schema.is_boolean() {
            problems.push(format!("{}: expected a JSON Schema object", at(pointer)));
        }
        return;
    };
    for (keyword, value) in object {
        match classify_keyword(keyword) {
            KeywordVerdict::Supported => {
                check_keyword_shape(keyword, value, pointer, depth, problems);
            }
            KeywordVerdict::Annotation => {}
            KeywordVerdict::Dialect => check_declared_dialect(value, pointer, problems),
            KeywordVerdict::NeverSupported(reason) => problems.push(format!(
                "{}: `{keyword}` is permanently unsupported: {reason}",
                at(pointer)
            )),
            KeywordVerdict::NotYetSupported(reason) => problems.push(format!(
                "{}: `{keyword}` is not supported yet: {reason}",
                at(pointer)
            )),
            KeywordVerdict::OtherDialect(dialect) => problems.push(format!(
                "{}: `{keyword}` is a JSON Schema {dialect} keyword, but this crate implements draft-07 only",
                at(pointer)
            )),
            KeywordVerdict::Unknown => problems.push(format!(
                "{}: unknown keyword `{keyword}` (not part of the draft-07 subset this crate enforces; check the spelling)",
                at(pointer)
            )),
        }
    }
}

/// 检查一个**受支持**关键字的取值形状；形状不对同样算「无法强制」。
///
/// 分两层是为了让每个函数的长度落在可读范围内：本层管**标量**关键字，凡是「形状里还有
/// 子 schema」的关键字一律交给 [`check_sub_schema_keyword`]。
fn check_keyword_shape(
    keyword: &str,
    value: &Value,
    pointer: &str,
    depth: usize,
    problems: &mut Vec<String>,
) {
    let here = at(pointer);
    match keyword {
        "type" => {
            if !is_valid_type(value) {
                problems.push(format!(
                    "{here}: malformed `type` (expected a JSON type name or an array of them)"
                ));
            }
        }
        "enum" => {
            if !value.is_array() {
                problems.push(format!("{here}: malformed `enum` (expected an array)"));
            }
        }
        "const" => {}
        "required" => {
            let well_formed = value
                .as_array()
                .is_some_and(|names| names.iter().all(Value::is_string));
            if !well_formed {
                problems.push(format!(
                    "{here}: malformed `required` (expected an array of property names)"
                ));
            }
        }
        "minProperties" | "maxProperties" | "minItems" | "maxItems" | "minLength" | "maxLength" => {
            if value.as_u64().is_none() {
                problems.push(format!(
                    "{here}: malformed `{keyword}` (expected a non-negative integer)"
                ));
            }
        }
        "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" => {
            if !value.is_number() {
                problems.push(format!(
                    "{here}: malformed `{keyword}` (draft-07 expects a number; the draft-04 boolean form is not supported)"
                ));
            }
        }
        "properties" | "additionalProperties" | "items" | "allOf" | "anyOf" | "oneOf" | "not" => {
            check_sub_schema_keyword(keyword, value, pointer, depth, problems);
        }
        _ => {
            // 只可能来自 SUPPORTED_KEYWORDS 与 match 臂不同步（缺陷）；保持静默会让形状检查漏掉。
            problems.push(format!("{here}: keyword `{keyword}` has no shape check"));
        }
    }
}

/// 检查「形状里含子 schema」的关键字，并递归到那些子 schema。
fn check_sub_schema_keyword(
    keyword: &str,
    value: &Value,
    pointer: &str,
    depth: usize,
    problems: &mut Vec<String>,
) {
    let here = at(pointer);
    match keyword {
        "properties" => match value.as_object() {
            None => problems.push(format!(
                "{here}: malformed `properties` (expected an object of schemas)"
            )),
            Some(properties) => {
                // 指针进到**本 schema 文档**里（`/properties/name`），不是实例路径：报错要
                // 能直接拿去定位写错的那个关键字。`validate_*` 一侧用 `/name` 是对的
                // （那里的指针指**实例**位置），两者刻意不同。
                let property_schemas = child_pointer(pointer, "properties");
                for (name, subschema) in properties {
                    walk_schema(
                        subschema,
                        &child_pointer(&property_schemas, name),
                        depth + 1,
                        problems,
                    );
                }
            }
        },
        "additionalProperties" => {
            if value.is_boolean() {
                // true / false 都是受支持形状（false = 禁止额外属性）。
            } else if value.is_object() {
                walk_schema(
                    value,
                    &child_pointer(pointer, "additionalProperties"),
                    depth + 1,
                    problems,
                );
            } else {
                problems.push(format!(
                    "{here}: malformed `additionalProperties` (expected a boolean or a schema)"
                ));
            }
        }
        "items" => {
            if value.is_object() || value.is_boolean() {
                walk_schema(value, &child_pointer(pointer, "items"), depth + 1, problems);
            } else {
                problems.push(format!(
                    "{here}: malformed `items` (tuple form is not supported; use a single schema)"
                ));
            }
        }
        "allOf" | "anyOf" | "oneOf" => match value.as_array() {
            None => problems.push(format!("{here}: malformed `{keyword}` (expected an array)")),
            Some(branches) => {
                if branches.is_empty() {
                    problems.push(format!("{here}: `{keyword}` must not be empty"));
                }
                for (index, branch) in branches.iter().enumerate() {
                    walk_schema(
                        branch,
                        &child_pointer_index(pointer, index),
                        depth + 1,
                        problems,
                    );
                }
            }
        },
        "not" => walk_schema(value, &child_pointer(pointer, "not"), depth + 1, problems),
        _ => {
            // 同上：只可能是 SUPPORTED_KEYWORDS 与 match 臂不同步（缺陷）。
            problems.push(format!(
                "{here}: keyword `{keyword}` has no sub-schema check"
            ));
        }
    }
}
/// `type` 的取值是否合法（类型名或类型名数组，名字必须在 [`JSON_TYPES`] 内）。
fn is_valid_type(value: &Value) -> bool {
    match value {
        Value::String(name) => JSON_TYPES.contains(&name.as_str()),
        Value::Array(names) => {
            !names.is_empty()
                && names
                    .iter()
                    .all(|name| name.as_str().is_some_and(|name| JSON_TYPES.contains(&name)))
        }
        _ => false,
    }
}

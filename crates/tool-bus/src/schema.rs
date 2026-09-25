//! JSON Schema（draft-07 **子集**）校验：**不支持即拒绝**（fail-closed）。
//!
//! ## 为什么手写最小子集（TASK-020 Q2 裁决，见任务卡 §5）
//!
//! `jsonschema` crate 会把 `borrow-or-share`（`MIT-0`）带进依赖图，而 `deny.toml` 白名单不含它
//! （放宽 = 漂移触发器 ⑥，需 ADR）；`rmcp` 自带校验经查**不存在**（泛型 `ServerHandler` 直接调用
//! `call_tool`，从不读 `get_tool`）。故选**零新增依赖**的自写子集。
//!
//! 判据方向很重要：**宁可让工具注册失败，也绝不放过一条自己无法强制的约束**。
//! 「不支持就放行」会把约束变成装饰，直接违反铁律 1 与铁律 4。
//!
//! ## 支持的子集（白名单见 [`SUPPORTED_KEYWORDS`]）
//!
//! 类型/取值 `type`/`enum`/`const`；对象 `properties`/`required`/`additionalProperties`/
//! `minProperties`/`maxProperties`；数组 `items`（**单 schema 形式**）/`minItems`/`maxItems`；
//! 字符串 `minLength`/`maxLength`（按 **Unicode 码点**计数）；数字 `minimum`/`maximum`/
//! `exclusiveMinimum`/`exclusiveMaximum`；组合 `allOf`/`anyOf`/`oneOf`/`not`。
//!
//! ## 忽略的注解（不算「支持」，但也不拒绝）
//!
//! `title`/`description`/`default`/`examples`/`$comment`/`deprecated`/`readOnly`/`writeOnly`/
//! `$id` —— 它们**不改变校验结果**，忽略是忠实的（`$id` 无 `$ref` 故无解析语义）。
//!
//! ## 永久放弃 / 尚未实现 / 其它方言
//!
//! 三张表逐条给出关键字 + 理由/方言 +（能给的）替代：永久放弃见 [`REJECTED_FOREVER_KEYWORDS`]
//! （`$ref`/`definitions`、`pattern`/`patternProperties`/`format`、`if`-`then`-`else`/
//! `dependencies`、元组 `items`/`additionalItems`、`contentEncoding`/`contentMediaType`），
//! 缺口见 [`REJECTED_FOR_NOW_KEYWORDS`]（`multipleOf`/`uniqueItems`/`contains`/`propertyNames`），
//! 其它草案见 [`NON_DRAFT07_KEYWORDS`]。**三张表之外的任何关键字 → 一律拒绝**（兜底同时挡拼写错误）。
//!
//! 替代方案一律是「`enum`/`const` 收窄取值」或「**由 handler 校验值**」—— 校验器管**形状**，
//! handler 管**值的语义安全**（参数化查询、路径规范化、不拼 shell）。
//!
//! ## `$schema` 不忽略，改做**方言校验**
//!
//! `$schema` 声明的是「这份文档按哪一版语义解释」：缺省 = draft-07，声明 draft-07 = 放行，
//! 声明**别的方言 = 拒绝**。否则我们会用 draft-07 的语义去校验一份 2020-12 的文档 —— 那正是
//! 铁律 1 禁止的「用自己的语义冒充别人的语义」。
//!
//! 相关：`docs/spec/tool-schema.md` §4 不变量 2 / 6 / **8**、架构 v2 §5.2、
//! `crates/tool-bus/README.md`「已知限制」、`docs/memory/rejected.md`。

use serde_json::{Map, Value};

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

/// schema / 实例的递归深度上限：对抗性输入不能把栈打穿。
const MAX_NESTING_DEPTH: usize = 64;

/// 一次校验最多报告几条违规（其余折叠成一行，免得把模型上下文塞满）。
const MAX_REPORTED_VIOLATIONS: usize = 8;

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

/// 校验一次工具调用参数；返回**空** Vec = 通过。
///
/// `arguments` 是已经解析好的 JSON 对象（MCP `tools/call` 的 `arguments`）。空对象与
/// `schema` 未声明 `required` 的组合是合法的（工具可以不收参数）。
///
/// 幂等 / 无副作用：纯函数；不调用 handler、不碰 IO。
#[must_use]
pub fn validate_arguments(schema: &Value, arguments: &Map<String, Value>) -> Vec<String> {
    // 为了复用同一套递归校验，把参数包装成 `Value::Object`。这是一次 O(参数规模) 的克隆，
    // 换来的是「对象 / 数组 / 标量」三种根形状共用一条实现；调用参数通常很小。
    let instance = Value::Object(arguments.clone());
    validate_instance(schema, &instance)
}

/// 通用入口：按 `schema` 校验任意 JSON 实例；返回**空** Vec = 通过。
fn validate_instance(schema: &Value, instance: &Value) -> Vec<String> {
    let mut violations = Vec::new();
    validate_at(schema, instance, "", 0, &mut violations);
    if violations.len() > MAX_REPORTED_VIOLATIONS {
        let omitted = violations.len() - MAX_REPORTED_VIOLATIONS;
        violations.truncate(MAX_REPORTED_VIOLATIONS);
        violations.push(format!("... and {omitted} more violation(s)"));
    }
    violations
}

/// 递归校验一个实例节点。
fn validate_at(
    schema: &Value,
    instance: &Value,
    pointer: &str,
    depth: usize,
    violations: &mut Vec<String>,
) {
    if depth > MAX_NESTING_DEPTH {
        violations.push(format!(
            "{}: validation nests deeper than {MAX_NESTING_DEPTH} levels",
            at(pointer)
        ));
        return;
    }
    match schema {
        Value::Bool(true) => {}
        Value::Bool(false) => {
            violations.push(format!("{}: rejected by a `false` schema", at(pointer)));
        }
        Value::Object(object) => {
            validate_object_schema(object, instance, pointer, depth, violations);
        }
        _ => violations.push(format!(
            "{}: schema is not an object (registration should have rejected this)",
            at(pointer)
        )),
    }
}

/// 校验一个「对象形状的 schema」对某个实例的全部约束。
fn validate_object_schema(
    schema: &Map<String, Value>,
    instance: &Value,
    pointer: &str,
    depth: usize,
    violations: &mut Vec<String>,
) {
    if let Some(declared_type) = schema.get("type")
        && !matches_declared_type(declared_type, instance)
    {
        // 类型都不对，再查下去只会刷屏；直接返回（组合子也跳过）。
        violations.push(format!(
            "{}: expected type `{}`, got {}",
            at(pointer),
            describe_declared_type(declared_type),
            describe_instance(instance)
        ));
        return;
    }
    if let Some(allowed) = schema.get("enum")
        && !allowed
            .as_array()
            .is_some_and(|candidates| candidates.contains(instance))
    {
        violations.push(format!("{}: value is not one of `enum`", at(pointer)));
    }
    if let Some(expected) = schema.get("const")
        && expected != instance
    {
        violations.push(format!("{}: value must equal `const`", at(pointer)));
    }
    if instance.is_number() {
        check_number_bounds(schema, instance, pointer, violations);
    }
    if let Some(text) = instance.as_str() {
        check_string_length(schema, text, pointer, violations);
    }
    if let Some(items) = instance.as_array() {
        check_array(schema, items, pointer, depth, violations);
    }
    if let Some(object) = instance.as_object() {
        check_object_properties(schema, object, pointer, depth, violations);
    }
    check_combinators(schema, instance, pointer, depth, violations);
}

/// 数值边界：`minimum` / `maximum` / `exclusiveMinimum` / `exclusiveMaximum`。
fn check_number_bounds(
    schema: &Map<String, Value>,
    instance: &Value,
    pointer: &str,
    violations: &mut Vec<String>,
) {
    let Some(value) = instance.as_f64() else {
        return;
    };
    let here = at(pointer);
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64)
        && value < minimum
    {
        violations.push(format!("{here}: {value} is below `minimum` {minimum}"));
    }
    if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64)
        && value > maximum
    {
        violations.push(format!("{here}: {value} is above `maximum` {maximum}"));
    }
    if let Some(minimum) = schema.get("exclusiveMinimum").and_then(Value::as_f64)
        && value <= minimum
    {
        violations.push(format!(
            "{here}: {value} must be > `exclusiveMinimum` {minimum}"
        ));
    }
    if let Some(maximum) = schema.get("exclusiveMaximum").and_then(Value::as_f64)
        && value >= maximum
    {
        violations.push(format!(
            "{here}: {value} must be < `exclusiveMaximum` {maximum}"
        ));
    }
}

/// 字符串长度（draft-07 按 **Unicode 码点**计数）。
fn check_string_length(
    schema: &Map<String, Value>,
    text: &str,
    pointer: &str,
    violations: &mut Vec<String>,
) {
    let here = at(pointer);
    let length = text.chars().count();
    if let Some(minimum) = schema.get("minLength").and_then(as_usize)
        && length < minimum
    {
        violations.push(format!(
            "{here}: string is shorter than `minLength` {minimum}"
        ));
    }
    if let Some(maximum) = schema.get("maxLength").and_then(as_usize)
        && length > maximum
    {
        violations.push(format!(
            "{here}: string is longer than `maxLength` {maximum}"
        ));
    }
}

/// 数组约束：`minItems` / `maxItems` / `items`。
fn check_array(
    schema: &Map<String, Value>,
    items: &[Value],
    pointer: &str,
    depth: usize,
    violations: &mut Vec<String>,
) {
    let here = at(pointer);
    if let Some(minimum) = schema.get("minItems").and_then(as_usize)
        && items.len() < minimum
    {
        violations.push(format!(
            "{here}: fewer than `minItems` {minimum} element(s)"
        ));
    }
    if let Some(maximum) = schema.get("maxItems").and_then(as_usize)
        && items.len() > maximum
    {
        violations.push(format!("{here}: more than `maxItems` {maximum} element(s)"));
    }
    if let Some(item_schema) = schema.get("items") {
        for (index, item) in items.iter().enumerate() {
            validate_at(
                item_schema,
                item,
                &child_pointer_index(pointer, index),
                depth + 1,
                violations,
            );
        }
    }
}

/// 对象约束：`minProperties` / `maxProperties` / `required` / `properties` / `additionalProperties`。
fn check_object_properties(
    schema: &Map<String, Value>,
    instance: &Map<String, Value>,
    pointer: &str,
    depth: usize,
    violations: &mut Vec<String>,
) {
    let here = at(pointer);
    if let Some(minimum) = schema.get("minProperties").and_then(as_usize)
        && instance.len() < minimum
    {
        violations.push(format!(
            "{here}: fewer than `minProperties` {minimum} property(ies)"
        ));
    }
    if let Some(maximum) = schema.get("maxProperties").and_then(as_usize)
        && instance.len() > maximum
    {
        violations.push(format!(
            "{here}: more than `maxProperties` {maximum} property(ies)"
        ));
    }
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            if !instance.contains_key(name) {
                violations.push(format!("{here}: missing required property `{name}`"));
            }
        }
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(properties) = properties {
        for (name, subschema) in properties {
            if let Some(value) = instance.get(name) {
                validate_at(
                    subschema,
                    value,
                    &child_pointer(pointer, name),
                    depth + 1,
                    violations,
                );
            }
        }
    }
    let additional = schema.get("additionalProperties");
    for (name, value) in instance {
        if properties.is_some_and(|properties| properties.contains_key(name)) {
            continue;
        }
        match additional {
            None | Some(Value::Bool(true)) => {}
            Some(Value::Bool(false)) => violations.push(format!(
                "{here}: additional property `{name}` is not allowed"
            )),
            Some(subschema) => validate_at(
                subschema,
                value,
                &child_pointer(pointer, name),
                depth + 1,
                violations,
            ),
        }
    }
}

/// 组合子：`allOf` / `anyOf` / `oneOf` / `not`。
fn check_combinators(
    schema: &Map<String, Value>,
    instance: &Value,
    pointer: &str,
    depth: usize,
    violations: &mut Vec<String>,
) {
    let here = at(pointer);
    if let Some(branches) = schema.get("allOf").and_then(Value::as_array) {
        for branch in branches {
            validate_at(branch, instance, pointer, depth + 1, violations);
        }
    }
    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array)
        && !branches
            .iter()
            .any(|branch| satisfies(branch, instance, depth + 1))
    {
        violations.push(format!(
            "{here}: value matches none of the `anyOf` branches"
        ));
    }
    if let Some(branches) = schema.get("oneOf").and_then(Value::as_array) {
        let matched = branches
            .iter()
            .filter(|branch| satisfies(branch, instance, depth + 1))
            .count();
        if matched != 1 {
            violations.push(format!(
                "{here}: value matches {matched} `oneOf` branch(es); exactly 1 is required"
            ));
        }
    }
    if let Some(branch) = schema.get("not")
        && satisfies(branch, instance, depth + 1)
    {
        violations.push(format!("{here}: value must not match the `not` schema"));
    }
}

/// 组合子只需要「满足 / 不满足」，因此跑一次丢弃输出的校验。
fn satisfies(schema: &Value, instance: &Value, depth: usize) -> bool {
    let mut sink = Vec::new();
    validate_at(schema, instance, "", depth, &mut sink);
    sink.is_empty()
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

/// 声明类型与实例是否匹配。
fn matches_declared_type(declared: &Value, instance: &Value) -> bool {
    match declared {
        Value::String(name) => instance_matches_type(name, instance),
        Value::Array(names) => names.iter().any(|name| {
            name.as_str()
                .is_some_and(|name| instance_matches_type(name, instance))
        }),
        _ => false,
    }
}

/// 单个类型名与实例的匹配（未知类型名 fail-closed）。
fn instance_matches_type(name: &str, instance: &Value) -> bool {
    match name {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "number" => instance.is_number(),
        "integer" => is_integer(instance),
        "boolean" => instance.is_boolean(),
        "null" => instance.is_null(),
        _ => false,
    }
}

/// draft-07 的 `integer`：整数值（`1.0` 也算，`1.5` 不算）。
fn is_integer(instance: &Value) -> bool {
    match instance {
        Value::Number(number) => {
            number.is_i64()
                || number.is_u64()
                || number.as_f64().is_some_and(|value| value.fract() == 0.0)
        }
        _ => false,
    }
}

/// 非负整数取值（`-1` / `1.5` / 字符串一律 None）。
fn as_usize(value: &Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
}

/// 声明类型的可读文本（只用于报错）。
fn describe_declared_type(declared: &Value) -> String {
    match declared {
        Value::String(name) => name.clone(),
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<&str>>()
            .join(" | "),
        _ => "<malformed>".to_owned(),
    }
}

/// 实例的 JSON 类型名（只用于报错）。
const fn describe_instance(instance: &Value) -> &'static str {
    match instance {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// JSON pointer 的一段（RFC 6901 转义：`~` → `~0`，`/` → `~1`）。
fn child_pointer(pointer: &str, name: &str) -> String {
    let escaped = name.replace('~', "~0").replace('/', "~1");
    format!("{pointer}/{escaped}")
}

/// JSON pointer 的数组下标段。
fn child_pointer_index(pointer: &str, index: usize) -> String {
    format!("{pointer}/{index}")
}

/// 空 pointer 的可读替身。
const fn at(pointer: &str) -> &str {
    if pointer.is_empty() {
        "<root>"
    } else {
        pointer
    }
}

//! JSON Schema（draft-07 **子集**）校验：**不支持即拒绝**（fail-closed）。
//!
//! ## 为什么手写最小子集（TASK-020 Q2 裁决，见任务卡 §5）
//!
//! 候选 `(a) jsonschema` crate 会把传递依赖 `borrow-or-share`（`MIT-0`）带进依赖图，
//! 而 `deny.toml` 的许可证白名单不含 `MIT-0` —— 放宽白名单是漂移触发器 ⑥，需要 ADR 与
//! 人类裁决。候选 `(b) rmcp 自带校验` 经查**不存在**：泛型 `ServerHandler` 路径直接调用
//! `call_tool`，从不读 `get_tool`；只有 `#[tool_router]` 宏生成的服务器才做参数处理，
//! 而且那是 schemars 的**类型驱动反序列化**，不是 draft-07 校验（本项目刻意不用宏路由，
//! 因为工具 schema 的事实源是 `assistant_protocol`，不是 Rust 类型）。
//! 因此本卡选 `(c)`：**零新增依赖** + **凡是不在支持清单里的关键字一律拒绝注册**。
//!
//! 判据的方向很重要：**宁可让工具注册失败，也绝不放过一条自己无法强制的约束**。
//! 「不支持就放行」会把约束变成装饰，直接违反铁律 1 与铁律 4。
//!
//! ## 支持的子集
//!
//! - 类型 / 取值：`type`（含类型数组）、`enum`、`const`
//! - 对象：`properties`、`required`、`additionalProperties`（bool 或子 schema）、
//!   `minProperties`、`maxProperties`
//! - 数组：`items`（**单 schema 形式**；元组形式不支持）、`minItems`、`maxItems`
//! - 字符串：`minLength`、`maxLength`（按 **Unicode 码点**计数，符合 draft-07）
//! - 数字：`minimum`、`maximum`、`exclusiveMinimum`、`exclusiveMaximum`（draft-07 的数值形式）
//! - 组合：`allOf`、`anyOf`、`oneOf`、`not`
//!
//! ## 刻意不做
//!
//! - 纯注解关键字（`title` / `description` / `default` / `examples` / `$comment` /
//!   `deprecated` / `readOnly` / `writeOnly` / `$schema` / `$id`）**忽略但不拒绝**：
//!   它们不影响校验结果。（`$id` 因为 `$ref` 不支持而没有解析语义。）
//! - 其余一切（`$ref` / `$defs` / `definitions` / `pattern` / `format` / `multipleOf` /
//!   `uniqueItems` / `patternProperties` / `propertyNames` / `dependencies` / `if`-`then`-
//!   `else` / `contains` / `prefixItems` / `unevaluatedProperties` …）→ **拒绝注册**并列出
//!   JSON pointer 路径。
//!
//! 相关：`docs/spec/tool-schema.md` §4 不变量 2 / 6、架构 v2 §5.2、`crates/tool-bus/README.md`
//! 「已知限制」。

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

/// 纯注解关键字：不影响校验结果，忽略（**不**拒绝）。
const ANNOTATION_KEYWORDS: &[&str] = &[
    "title",
    "description",
    "default",
    "examples",
    "$comment",
    "deprecated",
    "readOnly",
    "writeOnly",
    "$schema",
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
        if ANNOTATION_KEYWORDS.contains(&keyword.as_str()) {
            continue;
        }
        if !SUPPORTED_KEYWORDS.contains(&keyword.as_str()) {
            problems.push(format!("{}: unsupported keyword `{keyword}`", at(pointer)));
            continue;
        }
        check_keyword_shape(keyword, value, pointer, depth, problems);
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
                for (name, subschema) in properties {
                    walk_schema(
                        subschema,
                        &child_pointer(pointer, name),
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

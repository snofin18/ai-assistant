//! Runtime instance validation against the supported draft-07 subset.
//!
//! This module only evaluates already-registered schemas. Registration rejects
//! unsupported keywords and malformed schema shapes before this code is reached.

use serde_json::{Map, Value};

use super::shared::{MAX_NESTING_DEPTH, at, child_pointer, child_pointer_index};

/// Maximum number of violations returned for one instance.
const MAX_REPORTED_VIOLATIONS: usize = 8;

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

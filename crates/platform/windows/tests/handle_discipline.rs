//! 铁律 8 的机器校验（本 crate 侧）：**句柄不得获得跨进程能力**。
//!
//! 为什么需要它、以及为什么用"扫源码"而不是类型断言：Rust 没有稳定的"这个类型**没有**
//! 实现某 trait"断言（negative impl 不稳定）。而这里要防的失效模式很具体 ——
//! 有人为了"方便把目标传出去"给句柄加一行 `#[derive(Serialize)]`，或把 HWND 塞进一个
//! 可序列化的包装结构，句柄就悄悄获得了跨进程能力（架构 v2 §3.2 / §6.1）。
//! 扫源码能**精确**拦住这一行，代价是零依赖。同技术、同理由：
//! `crates/platform/api/tests/handles_not_serializable.rs`、`crates/core/tests/arch_layering.rs`。
//!
//! 与 `crates/platform/api` 那份的分工：**那边**守卫句柄的**定义**（`src/handle.rs`）；
//! **本文件**守卫本 crate —— 本 crate **不定义**句柄类型，只**组装**它们，所以这里要防的是
//! "在实现层新造一个可序列化的句柄 / 把平台对象搬进可序列化结构"。
//!
//! ## 不变量
//! 1. 断言**不是恒真**：先断言"真的扫到了预期的那些文件"，否则"目录改名 / 文件被删"
//!    会伪装成通过（铁律 1）。
//! 2. **负向用例必须存在**（ADR-0019 N1）：`scan_*` 喂已知坏样本，证明扫描器真会报。
//! 3. 扫描器**偏向误报而非漏报**：任何拿不准的形态都算"命中"。已知漏报面见 `strip_comments`。

use std::path::{Path, PathBuf};

/// 本 crate 里**最可能**长出句柄定义 / 句柄组装代码的文件。
///
/// 断言它们都在扫描结果里：路径改名会让"扫到 0 个文件"变成恒真通过。
const MUST_SCAN: &[&str] = &[
    "src/handles.rs",
    "src/lib.rs",
    "src/window/mod.rs",
    "src/uia/resolve.rs",
];

/// 出现这些词（**大小写不敏感**）就说明句柄获得了跨进程能力。
///
/// `serde` 是子串匹配，因此 `serde_json` / `serde::Serialize` 一并覆盖。
const FORBIDDEN_TOKENS: &[&str] = &["Serialize", "Deserialize", "serde", "to_json", "from_json"];

/// 期望扫到的源码文件数**下界**（2026-09-24 实测 17 个 `.rs`）。
///
/// 取一个明显低于实测值的数：它只用来证明"扫描真的发生了"，不是版本计数器
/// （把版本数写死在断言里，会让每次新增模块都得改测试）。
const MINIMUM_SOURCE_FILES: usize = 12;

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// 递归收集 `dir` 下所有 `.rs` 文件（相对 crate 根，**排序**后返回 → 失败可复现）。
fn collect_rust_sources(root: &Path, dir: &Path, collected: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(root, &path, collected);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            collected.push(relative);
        }
    }
    collected.sort();
}

/// 读一个源码文件；读不到就**失败**（绝不静默当成空文件 —— 那会让断言恒真）。
fn read_source(path: &Path) -> String {
    let outcome = std::fs::read_to_string(path);
    assert!(
        outcome.is_ok(),
        "必须能读到 {}（否则扫描会变成恒真）: {:?}",
        path.display(),
        outcome.as_ref().err()
    );
    let Ok(text) = outcome else {
        unreachable!("上面的 assert! 已保证是 Ok")
    };
    text
}

/// 只留**代码**：剥掉注释与字符串 / 字符字面量的**内容**（只保留一对引号占位）。
///
/// 处理：`//` 行注释（保留换行 → 行号仍对齐）、`/* */` 块注释、`"…"` 字符串（含 `\` 转义）、
/// `'x'` / `'\n'` 字符字面量。
///
/// **已知漏报面**（宁可误报也不要漏报，但如实写出来）：
/// - 块注释**不嵌套**：`/* /* */ */` 会提前结束，后面的注释文本被当成代码 → **误报**（安全方向）。
/// - **字符串字面量里的被禁词不算违规**（它只是文本，不产生任何能力）—— 这是**刻意**的，
///   否则本 crate 就没法在错误信息或测试数据里提到这些词。
/// - 未闭合的字面量会把文件剩余部分吞掉 → 漏报。这在合法 Rust 源码里不可能出现，
///   且真出现时 `rustc` 自己就会报错（不会静默进 main）。
fn strip_comments(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0_usize;
    let mut in_block_comment = false;
    let mut in_string = false;
    let mut escaped = false;

    while let Some(current) = chars.get(index).copied() {
        index += 1;
        if in_block_comment {
            if current == '*' && chars.get(index).copied() == Some('/') {
                index += 1;
                in_block_comment = false;
            }
            continue;
        }
        if in_string {
            // 内容整段丢弃（它不是代码，不产生任何能力）；只补回闭引号。
            if escaped {
                escaped = false;
            } else if current == '\\' {
                escaped = true;
            } else if current == '"' {
                in_string = false;
                out.push('"');
            }
            continue;
        }
        if current == '"' {
            in_string = true;
            out.push('"');
            continue;
        }
        if current == '\'' && is_char_literal_start(&chars, index) {
            // 内容同样丢弃：只保留一对引号占位（字面量里不可能出现被禁词）。
            out.push_str("''");
            while let Some(rest) = chars.get(index).copied() {
                index += 1;
                if rest == '\'' {
                    break;
                }
            }
            continue;
        }
        if current == '/' {
            if chars.get(index).copied() == Some('/') {
                // 行注释：丢到行尾，但**保留换行**，让后续行号仍然对得上。
                while let Some(rest) = chars.get(index).copied() {
                    index += 1;
                    if rest == '\n' {
                        out.push('\n');
                        break;
                    }
                }
                continue;
            }
            if chars.get(index).copied() == Some('*') {
                index += 1;
                in_block_comment = true;
                continue;
            }
        }
        out.push(current);
    }
    out
}

/// `chars[index]`（= 开头那个 `'` **之后**的字符）是不是一个字符字面量？
///
/// 用**两字符前瞻**区分「字符字面量」（`'x'` / `'\n'` / `'\''`）与「生命周期」
/// （`'a` / `'_` 后面不是闭引号）。用 `.get(i)` 而不是下标：workspace 把
/// `clippy::indexing_slicing` 定为 deny。
fn is_char_literal_start(chars: &[char], index: usize) -> bool {
    let first = chars.get(index).copied();
    let second = chars.get(index + 1).copied();
    let third = chars.get(index + 2).copied();
    matches!(
        (first, second, third),
        // `'\x'`：转义后的第三个字符才是闭引号。
        (Some('\\'), Some(_), Some('\'')) | (Some(_), Some('\''), _)
    )
}

/// 扫描一段源码，返回命中清单（空 = 干净）。**大小写不敏感**。
fn scan_code(source: &str) -> Vec<String> {
    let stripped = strip_comments(source);
    let mut hits = Vec::new();
    for (index, line) in stripped.lines().enumerate() {
        let lowered = line.to_lowercase();
        for token in FORBIDDEN_TOKENS {
            if lowered.contains(&token.to_lowercase()) {
                hits.push(format!("第 {} 行含 `{token}`：{}", index + 1, line.trim()));
            }
        }
    }
    hits
}

/// 扫描 `Cargo.toml`（剥掉 TOML 注释）：没有 `serde` 就没有 `derive(Serialize)` 可用。
fn scan_manifest(manifest: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (index, line) in manifest.lines().enumerate() {
        let code = line.split('#').next().unwrap_or("").to_lowercase();
        if code.contains("serde") {
            hits.push(format!("第 {} 行含 `serde`：{}", index + 1, line.trim()));
        }
    }
    hits
}

#[test]
fn test_no_source_file_grants_serialization_to_a_handle() {
    let root = crate_root();
    let mut sources = Vec::new();
    collect_rust_sources(&root, &root.join("src"), &mut sources);

    // 先证明"扫描真的发生了"（铁律 1：断言不得恒真）。
    assert!(
        sources.len() >= MINIMUM_SOURCE_FILES,
        "只扫到 {} 个 .rs 文件（期望 >= {MINIMUM_SOURCE_FILES}）—— 扫描器或目录结构变了",
        sources.len()
    );
    for required in MUST_SCAN {
        assert!(
            sources.iter().any(|path| path == Path::new(required)),
            "必须扫到 {required}（否则这条断言对句柄定义处是恒真的）"
        );
    }

    let mut violations = Vec::new();
    for relative in &sources {
        let source = read_source(&root.join(relative));
        for hit in scan_code(&source) {
            violations.push(format!("{}: {hit}", relative.display()));
        }
    }
    assert!(
        violations.is_empty(),
        "本 crate 不得新增可序列化的句柄 / 平台对象包装（铁律 8）：\n{}",
        violations.join("\n")
    );
}

#[test]
fn test_cargo_manifest_does_not_pull_in_serde() {
    let manifest = read_source(&crate_root().join("Cargo.toml"));
    assert!(
        manifest.contains("assistant-platform-api"),
        "扫到的文件不像本 crate 的 Cargo.toml —— 断言会变成恒真"
    );
    let hits = scan_manifest(&manifest);
    assert!(
        hits.is_empty(),
        "本 crate 不得引入 serde（引入它 = 句柄可以变成可序列化）：\n{}",
        hits.join("\n")
    );
}

#[test]
fn test_scanner_flags_a_synthetic_serializable_handle() {
    // 负向用例（ADR-0019 N1）：喂一个"有人顺手加了 derive"的坏样本，扫描器必须报。
    let bad = "#[derive(Serialize, Deserialize)]\npub struct HandleWrap { hwnd: isize }\n";
    let hits = scan_code(bad);
    assert!(
        !hits.is_empty(),
        "扫描器对已知坏样本必须报错（ADR-0019 N1）"
    );
    assert!(hits.iter().any(|hit| hit.contains("Serialize")));
    assert!(hits.iter().any(|hit| hit.contains("Deserialize")));
}

#[test]
fn test_scanner_flags_serde_import_and_json_helpers() {
    for bad in [
        "use serde::Serialize;\n",
        "use serde_json::json;\n",
        "fn to_json(&self) -> String { String::new() }\n",
        "fn from_json(text: &str) -> Self { let _ = text; Self }\n",
        // 大小写不敏感：小写形态同样是"给了句柄跨进程能力"。
        "fn serialize(&self) -> Vec<u8> { Vec::new() }\n",
    ] {
        assert!(!scan_code(bad).is_empty(), "`{bad}` 必须被扫描器拦住");
    }
}

#[test]
fn test_scanner_ignores_comments_and_string_literals() {
    // 正向对照：文档里解释"为什么不派生 Serialize"是**正常**的，不该误报。
    let clean = concat!(
        "//! 句柄类型**不派生** `Serialize` / `Deserialize`（铁律 8）。\n",
        "// 同上：serde 不是本 crate 的依赖。\n",
        "/* 块注释里的 serde 同样不算代码 */\n",
        "/// 文档注释里的 to_json / from_json 也不算。\n",
        "const NOTE: &str = \"说明文本里写 serde 也不算代码\";\n",
        "pub struct LocalHandleId { value: u64 }\n",
    );
    assert_eq!(
        scan_code(clean),
        Vec::<String>::new(),
        "注释 / 字符串字面量里的词不得被当成违规代码"
    );
}

#[test]
fn test_scanner_keeps_line_numbers_aligned_after_stripping_comments() {
    // 行号对齐是"失败信息可用"的前提：剥掉注释后命中行号必须仍是**源码行号**。
    let source = "// 注释一行\n// 又一行\n#[derive(Serialize)]\n";
    let hits = scan_code(source);
    assert_eq!(hits.len(), 1);
    assert!(
        hits.first().is_some_and(|hit| hit.starts_with("第 3 行")),
        "命中行号必须是源码里的第 3 行，实际 = {hits:?}"
    );
}

#[test]
fn test_manifest_scanner_flags_a_synthetic_serde_dependency() {
    // 负向用例：manifest 扫描器必须对 `serde = …` 报，且不得对注释里的 serde 报。
    let bad = "[dependencies]\nserde = { version = \"1\", features = [\"derive\"] }\n";
    assert!(!scan_manifest(bad).is_empty(), "serde 依赖必须被拦住");
    let commented = "# 说明：本 crate 不用 serde\n[dependencies]\n";
    assert!(
        scan_manifest(commented).is_empty(),
        "TOML 注释里的 serde 不该误报"
    );
}

//! 铁律 8 的机器校验：**句柄不得跨进程**。
//!
//! 为什么用"扫源码"而不是类型断言：Rust 没有"这个类型**没有**实现某 trait"的稳定断言
//! （negative impl 不稳定）。而这里要防的失效模式很具体 ——
//! 有人为了图方便给 `ResolvedWindow` / `ResolvedElement` 加一行 `#[derive(Serialize)]`，
//! 于是句柄悄悄获得了跨进程能力。扫源码能**精确**拦住这一行，代价是零依赖。
//!
//! 与 `crates/core/tests/arch_layering.rs`（gov §5.1 门禁 #5）同技术、同理由。
//!
//! ## 不变量
//! 1. 断言不是恒真：先断言"真的读到了句柄定义文件"，否则"文件不存在"会伪装成通过（铁律 1）
//! 2. **负向用例必须存在**（ADR-0019 N1）：`scan_flags_*` 喂已知坏样本，证明扫描器真会报

mod common;

use common::ok_or_fail;
use std::path::{Path, PathBuf};

/// 句柄定义文件（本 crate 内**唯一**允许定义句柄的地方）。
const HANDLE_SOURCE: &str = "src/handle.rs";

/// 出现这些词就说明句柄获得了跨进程能力。
const FORBIDDEN_TOKENS: &[&str] = &["Serialize", "Deserialize", "serde", "to_json", "from_json"];

/// 扫描一段源码；返回命中清单（空 = 干净）。
fn scan_handle_source(source: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (index, line) in source.lines().enumerate() {
        // 先剥掉 `//` 之后的注释：文档里解释"为什么不派生 Serialize"是**正常**的，
        // 真正违规的是代码里出现它（与 arch_layering.rs 同一处理）。
        let code = line.split("//").next().unwrap_or("");
        for token in FORBIDDEN_TOKENS {
            if code.contains(token) {
                hits.push(format!("第 {} 行含 `{token}`：{}", index + 1, line.trim()));
            }
        }
    }
    hits
}

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

#[test]
fn test_handle_source_has_no_serialization_tokens() {
    let path = crate_root().join(HANDLE_SOURCE);
    let source = ok_or_fail(
        std::fs::read_to_string(&path),
        &format!("句柄定义文件必须存在且可读: {}", path.display()),
    );
    assert!(
        source.contains("ResolvedWindow") && source.contains("ResolvedElement"),
        "扫到的文件里没有句柄定义 —— 断言会变成恒真（铁律 1）"
    );
    let hits = scan_handle_source(&source);
    assert!(
        hits.is_empty(),
        "句柄不得获得序列化能力（铁律 8）：\n{}",
        hits.join("\n")
    );
}

#[test]
fn test_scan_flags_a_synthetic_serializable_handle() {
    // 负向用例：喂一个"有人顺手加了 derive"的坏样本，扫描器必须报。
    let bad = "#[derive(Serialize)]\npub struct ResolvedWindow { id: u64 }\n";
    let hits = scan_handle_source(bad);
    assert!(
        !hits.is_empty(),
        "扫描器对已知坏样本必须报错（ADR-0019 N1）"
    );
    assert!(hits.iter().any(|hit| hit.contains("Serialize")));
}

#[test]
fn test_scan_flags_serde_import() {
    let bad = "use serde::Serialize;\n";
    let hits = scan_handle_source(bad);
    assert!(!hits.is_empty(), "`use serde::...` 同样是违规");
}

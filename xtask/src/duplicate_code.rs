//! # 跨文件重复代码规则（gov §5.4 / ADR-0068）
//!
//! 职责：对合格 Rust 源文件生成规范化 token shingle，找出近似复制粘贴的文件对。
//! 规则是纯函数：输入 `(相对路径, 源码文本)`，输出稳定的 `Vec<Finding>`。
//!
//! ## 边界（不做什么）
//! - 不做文件 IO、不读生成物、不访问网络。
//! - 不做 AST 解析：xtask 保持零第三方依赖，本模块只做有界的词法规范化。
//! - 不自动改写或提供源码内豁免；阈值与忽略集只能由 ADR 修改。
//!
//! ## 不变量
//! 1. 输入顺序不影响结果：文件按相对路径排序后处理。
//! 2. 每个无序文件对最多产出一条 Warning。
//! 3. 扫描有硬上限；超限必须产生显式 Warning，不得静默截断。
//! 4. 哈希、阈值、输出顺序确定，跨平台得到同一结果。
//!
//! 相关：`docs/adr/0068-cross-file-duplicate-code-hygiene.md`

use std::collections::BTreeMap;

use crate::report::{Finding, Severity};

/// 一个 shingle 覆盖的连续 token 数。
pub const DUPLICATE_SHINGLE_TOKENS: usize = 40;

/// 触发 Warning 的最低包含度（百分比）。
pub const DUPLICATE_SIMILARITY_WARN_PERCENT: usize = 80;

/// 触发 Warning 的最少共享 distinct shingle 数。
pub const DUPLICATE_MIN_SHARED_SHINGLES: usize = 4;

/// 参与规则的文件最少 token 数。
pub const DUPLICATE_MIN_TOKENS: usize = 80;

/// 单文件字节上限（1 MiB）。
pub const DUPLICATE_MAX_FILE_BYTES: usize = 1_048_576;

/// 最多扫描的合格文件数。
pub const DUPLICATE_MAX_FILES: usize = 1_000;

/// 全局最多生成的 shingle 数。
pub const DUPLICATE_MAX_SHINGLES: usize = 1_000_000;

/// 一个 shingle 出现在超过该文件数时视为公共样板并跳过。
pub const DUPLICATE_MAX_SHINGLE_FILE_OCCURRENCES: usize = 8;

/// 一条待比较的 Rust 源文件。
#[derive(Debug, Clone, Copy)]
pub struct DuplicateSource<'a> {
    /// 相对仓库根、以 `/` 分隔的路径。
    pub relative_path: &'a str,
    /// 源码文本。
    pub source: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceToken {
    text: String,
    line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint {
    relative_path: String,
    first_line_by_hash: BTreeMap<u64, usize>,
}

#[derive(Debug, Default)]
struct PairStats {
    shared_shingles: usize,
    first_line: usize,
}

/// 执行跨文件重复代码检查。
#[must_use]
pub fn check_duplicate_code(sources: &[DuplicateSource<'_>]) -> Vec<Finding> {
    let mut eligible: Vec<&DuplicateSource<'_>> = sources
        .iter()
        .filter(|source| !is_ignored_duplicate_path(source.relative_path))
        .collect();
    eligible.sort_by_key(|source| source.relative_path);

    let mut fingerprints = Vec::new();
    let mut total_shingles = 0usize;
    let mut truncated = false;
    for source in eligible.iter().take(DUPLICATE_MAX_FILES) {
        if source.source.len() > DUPLICATE_MAX_FILE_BYTES {
            truncated = true;
            continue;
        }
        let Some(fingerprint) = fingerprint_source(source) else {
            continue;
        };
        if total_shingles.saturating_add(fingerprint.first_line_by_hash.len())
            > DUPLICATE_MAX_SHINGLES
        {
            truncated = true;
            break;
        }
        total_shingles += fingerprint.first_line_by_hash.len();
        fingerprints.push(fingerprint);
    }
    if eligible.len() > DUPLICATE_MAX_FILES {
        truncated = true;
    }

    let mut findings = duplicate_findings(&fingerprints);
    if truncated {
        findings.push(Finding::new(
            "hygiene/duplicate-scan-truncated",
            Severity::Warning,
            "xtask",
            0,
            format!(
                "重复代码扫描达到硬上限（最多 {DUPLICATE_MAX_FILES} 文件 / {DUPLICATE_MAX_SHINGLES} shingle / 单文件 {DUPLICATE_MAX_FILE_BYTES} 字节），结果可能不完整（ADR-0068 D9）"
            ),
        ));
    }
    findings
}

/// 路径是否属于 ADR-0068 的忽略集。
#[must_use]
pub fn is_ignored_duplicate_path(relative_path: &str) -> bool {
    relative_path.starts_with("fixtures/")
        || relative_path.starts_with("spikes/")
        || relative_path.starts_with("tools/")
        || relative_path.starts_with("tests/")
        || relative_path.contains("/tests/")
        || relative_path.contains("/generated/")
        || relative_path.starts_with("crates/protocol/src/generated/")
        || relative_path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.ends_with("_tests.rs"))
}

fn fingerprint_source(source: &DuplicateSource<'_>) -> Option<FileFingerprint> {
    let tokens = tokenize(source.source);
    if tokens.len() < DUPLICATE_MIN_TOKENS {
        return None;
    }
    let window_count = tokens
        .len()
        .checked_sub(DUPLICATE_SHINGLE_TOKENS)?
        .saturating_add(1);
    let mut first_line_by_hash = BTreeMap::new();
    for start in 0..window_count {
        let end = start.checked_add(DUPLICATE_SHINGLE_TOKENS)?;
        let window = tokens.get(start..end)?;
        let line = tokens.get(start)?.line;
        let hash = hash_tokens(window);
        first_line_by_hash.entry(hash).or_insert(line);
    }
    (first_line_by_hash.len() >= DUPLICATE_MIN_SHARED_SHINGLES).then(|| FileFingerprint {
        relative_path: source.relative_path.to_string(),
        first_line_by_hash,
    })
}

fn duplicate_findings(fingerprints: &[FileFingerprint]) -> Vec<Finding> {
    let mut by_hash: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (file_index, fingerprint) in fingerprints.iter().enumerate() {
        for hash in fingerprint.first_line_by_hash.keys() {
            by_hash.entry(*hash).or_default().push(file_index);
        }
    }

    let mut pairs: BTreeMap<(usize, usize), PairStats> = BTreeMap::new();
    for (hash, file_indices) in &by_hash {
        if file_indices.len() < 2 || file_indices.len() > DUPLICATE_MAX_SHINGLE_FILE_OCCURRENCES {
            continue;
        }
        for (offset, left_index) in file_indices.iter().enumerate() {
            for right_index in file_indices.iter().skip(offset.saturating_add(1)) {
                if left_index >= right_index {
                    continue;
                }
                let line = fingerprints
                    .get(*left_index)
                    .and_then(|fingerprint| fingerprint.first_line_by_hash.get(hash))
                    .copied()
                    .unwrap_or(0);
                let stats = pairs.entry((*left_index, *right_index)).or_default();
                stats.shared_shingles = stats.shared_shingles.saturating_add(1);
                if stats.first_line == 0 || line < stats.first_line {
                    stats.first_line = line;
                }
            }
        }
    }

    pairs
        .into_iter()
        .filter_map(|((left_index, right_index), stats)| {
            let left = fingerprints.get(left_index)?;
            let right = fingerprints.get(right_index)?;
            let minimum = left
                .first_line_by_hash
                .len()
                .min(right.first_line_by_hash.len());
            if !meets_similarity_threshold(stats.shared_shingles, minimum) {
                return None;
            }
            let percent = stats
                .shared_shingles
                .saturating_mul(100)
                .checked_div(minimum)?;
            Some(Finding::new(
                "hygiene/duplicate-code",
                Severity::Warning,
                &left.relative_path,
                stats.first_line,
                format!(
                    "与 {} 的规范化 token 指纹包含度 {percent}%（共享 {}/{}），疑似跨文件复制粘贴；考虑抽共享函数或由 ADR 调整阈值（ADR-0068）",
                    right.relative_path, stats.shared_shingles, minimum
                ),
            ))
        })
        .collect()
}

const fn meets_similarity_threshold(shared_shingles: usize, minimum_shingles: usize) -> bool {
    minimum_shingles > 0
        && shared_shingles >= DUPLICATE_MIN_SHARED_SHINGLES
        && shared_shingles.saturating_mul(100)
            >= DUPLICATE_SIMILARITY_WARN_PERCENT.saturating_mul(minimum_shingles)
}

fn hash_tokens(tokens: &[SourceToken]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for token in tokens {
        for byte in token.text.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn tokenize(source: &str) -> Vec<SourceToken> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    let mut line = 1usize;
    while index < bytes.len() {
        let Some(byte) = bytes.get(index).copied() else {
            break;
        };
        if byte == b'\n' {
            line += 1;
            index += 1;
        } else if byte.is_ascii_whitespace() {
            index += 1;
        } else if let Some(skipped) = skip_non_code(bytes, index) {
            if skipped.emits_literal {
                tokens.push(SourceToken {
                    text: "literal".to_string(),
                    line,
                });
            }
            index = skipped.next_index;
            line += skipped.newlines;
        } else if is_identifier_start(byte) {
            index = push_identifier_token(source, bytes, index, line, &mut tokens);
        } else if byte.is_ascii_digit() {
            let end = consume_number(bytes, index);
            tokens.push(SourceToken {
                text: "number".to_string(),
                line,
            });
            index = end;
        } else {
            index = push_punctuation_token(source, index, line, &mut tokens);
        }
    }
    tokens
}

fn push_identifier_token(
    source: &str,
    bytes: &[u8],
    index: usize,
    line: usize,
    tokens: &mut Vec<SourceToken>,
) -> usize {
    let end = consume_identifier(bytes, index);
    let raw = source.get(index..end).unwrap_or_default();
    tokens.push(SourceToken {
        text: if keyword(raw) {
            raw.to_string()
        } else {
            "identifier".to_string()
        },
        line,
    });
    end
}

fn push_punctuation_token(
    source: &str,
    index: usize,
    line: usize,
    tokens: &mut Vec<SourceToken>,
) -> usize {
    let character = source
        .get(index..)
        .and_then(|tail| tail.chars().next())
        .unwrap_or('\0');
    tokens.push(SourceToken {
        text: character.to_string(),
        line,
    });
    index.saturating_add(character.len_utf8())
}

#[derive(Debug, Clone, Copy)]
struct SkippedNonCode {
    next_index: usize,
    newlines: usize,
    emits_literal: bool,
}

fn skip_non_code(bytes: &[u8], index: usize) -> Option<SkippedNonCode> {
    let byte = bytes.get(index).copied()?;
    if starts_with(bytes, index, b"//") {
        return Some(SkippedNonCode {
            next_index: skip_line_comment(bytes, index),
            newlines: 0,
            emits_literal: false,
        });
    }
    if starts_with(bytes, index, b"/*") {
        let (next_index, newlines) = skip_block_comment(bytes, index);
        return Some(SkippedNonCode {
            next_index,
            newlines,
            emits_literal: false,
        });
    }
    if let Some((next_index, newlines)) = skip_raw_string(bytes, index) {
        return Some(SkippedNonCode {
            next_index,
            newlines,
            emits_literal: true,
        });
    }
    if byte == b'"' || (byte == b'b' && bytes.get(index + 1) == Some(&b'"')) {
        let quote_index = if byte == b'"' { index } else { index + 1 };
        let (next_index, newlines) = skip_quoted_literal(bytes, quote_index, b'"');
        return Some(SkippedNonCode {
            next_index,
            newlines,
            emits_literal: true,
        });
    }
    if byte == b'\'' || (byte == b'b' && bytes.get(index + 1) == Some(&b'\'')) {
        let quote_index = if byte == b'\'' { index } else { index + 1 };
        let (next_index, newlines) = skip_char_literal(bytes, quote_index)?;
        return Some(SkippedNonCode {
            next_index,
            newlines,
            emits_literal: true,
        });
    }
    None
}

fn starts_with(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes
        .get(index..)
        .is_some_and(|tail| tail.starts_with(needle))
}

fn skip_line_comment(bytes: &[u8], index: usize) -> usize {
    let mut cursor = index.saturating_add(2);
    while cursor < bytes.len() && bytes.get(cursor) != Some(&b'\n') {
        cursor += 1;
    }
    cursor
}

fn skip_block_comment(bytes: &[u8], index: usize) -> (usize, usize) {
    let mut cursor = index;
    let mut depth = 0usize;
    let mut newlines = 0usize;
    while cursor < bytes.len() {
        if starts_with(bytes, cursor, b"/*") {
            depth += 1;
            cursor += 2;
        } else if starts_with(bytes, cursor, b"*/") {
            depth = depth.saturating_sub(1);
            cursor += 2;
            if depth == 0 {
                break;
            }
        } else if bytes.get(cursor) == Some(&b'\n') {
            newlines += 1;
            cursor += 1;
        } else {
            cursor += 1;
        }
    }
    (cursor, newlines)
}

fn skip_raw_string(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    let mut cursor = index;
    if bytes.get(cursor) == Some(&b'b') {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let mut hashes = 0usize;
    while bytes.get(cursor) == Some(&b'#') {
        hashes += 1;
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'"') {
        return None;
    }
    cursor += 1;
    let mut newlines = 0usize;
    while cursor < bytes.len() {
        if bytes.get(cursor) == Some(&b'\n') {
            newlines += 1;
        }
        if bytes.get(cursor) == Some(&b'"')
            && bytes
                .get(cursor + 1..cursor + 1 + hashes)
                .is_some_and(|tail| tail.iter().all(|byte| *byte == b'#'))
        {
            return Some((cursor + 1 + hashes, newlines));
        }
        cursor += 1;
    }
    Some((cursor, newlines))
}

fn skip_quoted_literal(bytes: &[u8], quote_index: usize, quote: u8) -> (usize, usize) {
    let mut cursor = quote_index.saturating_add(1);
    let mut newlines = 0usize;
    while cursor < bytes.len() {
        let Some(byte) = bytes.get(cursor).copied() else {
            break;
        };
        if byte == b'\\' {
            cursor = cursor.saturating_add(2);
        } else if byte == quote {
            cursor += 1;
            break;
        } else {
            if byte == b'\n' {
                newlines += 1;
            }
            cursor += 1;
        }
    }
    (cursor, newlines)
}

fn skip_char_literal(bytes: &[u8], quote_index: usize) -> Option<(usize, usize)> {
    let (next_index, newlines) = skip_quoted_literal(bytes, quote_index, b'\'');
    (next_index > quote_index.saturating_add(1) && newlines == 0).then_some((next_index, newlines))
}

const fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn consume_identifier(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start;
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        cursor += 1;
    }
    cursor
}

fn consume_number(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start;
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'.')
    {
        cursor += 1;
    }
    cursor
}

fn keyword(raw: &str) -> bool {
    const KEYWORDS: [&str; 51] = [
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while", "yield", "box", "macro", "union", "dyn",
        "abstract", "become", "do", "final", "override", "priv", "typeof", "unsized",
    ];
    KEYWORDS.contains(&raw)
}

#[cfg(test)]
#[path = "duplicate_code_tests.rs"]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

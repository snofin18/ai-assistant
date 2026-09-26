//! # 文件改写互斥锁的纯逻辑（ADR-0028 的判定与编解码一半）
//!
//! 职责：定义锁记录的**文本编解码**、锁文件名的**派生规则**、以及「该不该拿到这把锁」的
//! **判定函数** [`decide_acquire`]。全部是纯函数。
//!
//! 文件 IO（`create_new` 原子创建、轮询睡眠、放弃日志追加）在 `guard_runner.rs`，分派与退出码在
//! `main.rs`。切分理由同其它模块：**判定与编解码可被逐分支白盒测试，不需要临时目录与真实多进程时序**。
//!
//! ## 为什么需要这把锁
//! 本项目的默认工况是「多个 AI agent 会话 + subagent 并行 + 夜间无人值守」，
//! `AGENTS.md` §8 明确允许并行度 ≤3。现有两道防线都不够：write scope 是**文档约定**，
//! 读→改→写之间没有原子性，两个 agent 各自基于旧内容写回就是经典的 **lost update**，
//! 而 git **不会报冲突**（两次写都"成功"了）；章程 §11.2 的 `.automation.lock` 粒度是整个仓库且只在
//! 夜间生效。风险最高的恰是 `LEDGER.md` / `docs/memory/*` / `docs/PARKING_LOT.md` 这三个
//! **只追加的公共热点文件** —— 覆盖别人追加的那一行，没有任何工具会报错。
//!
//! ## 边界（不做什么）
//! - 不做 IO、不读时钟、不睡眠：`now_unix` 与所有阈值都由调用方注入（保证可回放）。
//! - **不做 PID 存活探测**（ADR-0028 D6）：那需要 `windows` crate 或 `libc`，违反 xtask
//!   不变量 1（零第三方依赖）；而 PID 复用本身还会造成误判（旧 PID 被无关进程占用 →
//!   判定"还活着" → 永久死锁）。陈旧锁只用**时间**判据。
//! - 不做强制：协作式锁**无法技术强制**（ADR-0028 风险表已承认这是本方案唯一的真实弱点），
//!   绕过它由 `AGENTS.md` 义务 + review agent + `memory-counts`/`adr-index` 事后发现来兜。
//!
//! ## 不变量
//! 1. `slug_for` 与 `fnv1a_hex32` 是纯函数且**跨平台一致**（同样的路径必得同样的锁文件名）。
//! 2. `parse_lock_record(render_lock_record(record))` 必须还原出等价的记录（编解码互逆）。
//! 3. 锁记录的 `intent` / `task` 字段**不得含换行**（渲染时替换为空格），否则行式格式会被破坏。
//! 4. 判定不吞异常：锁记录解析失败**不由本模块兜底**，调用方必须显式处理（铁律 1）。
//!
//! 相关：`docs/adr/0028-file-rewrite-mutex-protocol.md`、`xtask/src/guard_runner.rs`

/// 锁文件目录（相对仓库根）。刻意放在 `target/` 下：`.gitignore` 的 `/target/` 已覆盖它，
/// 锁文件与放弃日志**都不入库**。代价是 `cargo clean` 会把锁全删掉（fail-open）——
/// ADR-0028 风险表已明确接受：write scope + review 才是主防线，guard 是第二道。
pub const LOCK_DIR: &str = "target/locks";

/// 放弃日志路径（相对仓库根）。超时放弃与陈旧接管都会往这里追加一行，供事后取证。
pub const ABANDONMENT_LOG: &str = "target/locks/abandonments.log";

/// 等待锁的默认超时（秒）。
///
/// 30 秒对「读-改-写」足够（一次写回是毫秒级）；长任务应显式传更大的 `--timeout`，
/// 而不是把默认值调大（ADR-0028 风险表）。
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// 判定锁陈旧的默认阈值（秒）。到期即可接管（`--stale-after`）。
pub const DEFAULT_STALE_AFTER_SECS: u64 = 900;

/// 轮询间隔（毫秒）。
pub const POLL_INTERVAL_MILLIS: u64 = 100;

/// 锁文件扩展名。抽成常量的理由：`slug_for`（派生锁名）与 `guard_store::list`（扫描锁目录）
/// 必须用**同一个**字面量，否则一侧改了另一侧会静默失配；同时满足 clippy 对「扩展名比较必须
/// 大小写无关」的要求（两侧都在比较前转小写）。
pub const LOCK_FILE_SUFFIX: &str = ".lock";

/// slug 的可见部分最长字符数（超出截断；重名由后面的摘要兜住）。
const SLUG_VISIBLE_LIMIT: usize = 64;

/// 一条锁记录（ADR-0028 D4：行式 `key=value` 文本，不是 JSON）。
///
/// 为什么不用 JSON：手写 JSON 解析器的转义坑（引号、反斜杠、多字节）比行式格式多得多，
/// 而 xtask 零第三方依赖，没有 `serde` 可用。
/// **为什么必须带人类可读字段**（owner / task / intent / iso 时间）：超时放弃时，
/// 等待方要能说出「谁、什么时候、为了什么」拿着这把锁 —— 只有 PID 是无法诊断的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockRecord {
    /// 被锁定的仓库相对路径（统一用 `/` 分隔）。
    pub target: String,
    /// 持有者标识，**必须会话级唯一**（`AGENTS.md` 规定：`<agent>-<thread 前 8 位>` 或 `TASK-NNN`）。
    pub owner: String,
    /// 持有者进程 id（仅作诊断线索，**不参与存活判定**，见模块头的「边界」）。
    pub pid: u32,
    /// 关联的任务卡号（无则空串）。
    pub task: String,
    /// 获取时刻的 Unix 秒（陈旧判定的唯一依据）。
    pub acquired_at_unix: u64,
    /// 获取时刻的 ISO-8601 文本（给人看的）。
    pub acquired_at_iso: String,
    /// 这次改写想干什么（一句话）。
    pub intent: String,
}

/// 锁记录里必须出现的键（顺序即渲染顺序）。
const RECORD_KEYS: [&str; 7] = [
    "target",
    "owner",
    "pid",
    "task",
    "acquired_at_unix",
    "acquired_at_iso",
    "intent",
];

/// 「拿到这把锁」的判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Acquisition {
    /// 没有现存锁记录 → 可以原子创建。
    Grant,
    /// 现存锁的 owner 与请求者相同 → 复用（同一会话重复 acquire 不该被自己挡住）。
    Reuse,
    /// 他人持有且仍新鲜 → 等待。
    Wait {
        /// 当前持有者的记录（用于诊断输出）。
        held: LockRecord,
    },
    /// 他人持有但已陈旧，或请求者显式 `--force` → 接管。
    TakeOver {
        /// 被接管者的记录（**必须**打印并写进放弃日志，不静默）。
        held: LockRecord,
        /// 接管理由。
        reason: TakeOverReason,
    },
    /// 锁记录的获取时刻在**未来**（时钟回拨或对方时钟错误）。
    ///
    /// 此时判为「等待」而不是「陈旧」：一次时钟调整就把所有锁判成陈旧并集体接管，
    /// 比多等一会儿危险得多（ADR-0028 风险表）。
    ClockAnomaly {
        /// 当前持有者的记录。
        held: LockRecord,
    },
}

/// 接管一把锁的理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeOverReason {
    /// 锁龄 ≥ `--stale-after`（ADR-0028 D6）。
    Stale,
    /// 请求者显式传了 `--force`（人工接管，不受陈旧阈值限制）。
    Forced,
}

impl TakeOverReason {
    /// 用于机器可读输出与放弃日志的短标签。
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stale => "STALE",
            Self::Forced => "FORCED",
        }
    }
}

/// 渲染锁记录为行式 `key=value` 文本（末尾带一个 `\n`）。
///
/// 不变量 3：`task` 与 `intent` 里的换行会被替换成空格 —— 行式格式一旦混入换行，
/// 解析就会把后半截当成未知键，锁记录随之失效。
#[must_use]
pub fn render_lock_record(record: &LockRecord) -> String {
    let mut text = String::new();
    for (key, value) in RECORD_KEYS.iter().zip(record_fields(record)) {
        text.push_str(key);
        text.push('=');
        text.push_str(&flatten_single_line(&value));
        text.push('\n');
    }
    text
}

/// 按 [`RECORD_KEYS`] 的顺序取出记录的七个字段值。
fn record_fields(record: &LockRecord) -> [String; 7] {
    [
        record.target.clone(),
        record.owner.clone(),
        record.pid.to_string(),
        record.task.clone(),
        record.acquired_at_unix.to_string(),
        record.acquired_at_iso.clone(),
        record.intent.clone(),
    ]
}

/// 把换行与回车压成单个空格，保证「一个字段一行」。
///
/// 连续换行（含 Windows 的 `\r\n`）只产生**一个**空格：逐字符替换会让 `"a\r\nb"`
/// 变成 `"a  b"`（两个空格），与本行文档承诺的"单个空格"不符，也会让人误以为原文有两个空格。
/// 只吃掉换行符组成的连续段，**不**动用户自己敲的普通空格。
fn flatten_single_line(value: &str) -> String {
    let mut flattened = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\r' || character == '\n' {
            // 吃掉整段连续换行，只留一个空格
            while let Some(&upcoming) = characters.peek() {
                if upcoming != '\r' && upcoming != '\n' {
                    break;
                }
                characters.next();
            }
            flattened.push(' ');
        } else {
            flattened.push(character);
        }
    }
    flattened.trim().to_string()
}

/// 解析锁记录文本。
///
/// 严格程度是刻意的：七个键**必须全部出现**，`pid` 与 `acquired_at_unix` 必须是合法整数。
/// 半截写入（进程写锁时被 kill）会返回 `Err`，调用方必须显式处理，**不得**把「解析不出来」
/// 当成「没有锁」（那会让两个进程同时认为自己拿到了锁）。
///
/// # Errors
/// 缺少任一键、或数值字段解析失败时返回带原因的 `Err`。
pub fn parse_lock_record(text: &str) -> Result<LockRecord, String> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            return Err(format!("锁记录有一行不是 `key=value` 形态：`{trimmed}`"));
        };
        pairs.push((key.trim().to_string(), value.trim().to_string()));
    }
    let lookup = |key: &str| -> Result<String, String> {
        pairs
            .iter()
            .find(|(found_key, _)| found_key == key)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| format!("锁记录缺少 `{key}` 字段"))
    };
    let pid_text = lookup("pid")?;
    let unix_text = lookup("acquired_at_unix")?;
    let pid = pid_text
        .parse::<u32>()
        .map_err(|error| format!("锁记录的 `pid` 不是整数（`{pid_text}`）：{error}"))?;
    let acquired_at_unix = unix_text.parse::<u64>().map_err(|error| {
        format!("锁记录的 `acquired_at_unix` 不是整数（`{unix_text}`）：{error}")
    })?;
    Ok(LockRecord {
        target: lookup("target")?,
        owner: lookup("owner")?,
        pid,
        task: lookup("task")?,
        acquired_at_unix,
        acquired_at_iso: lookup("acquired_at_iso")?,
        intent: lookup("intent")?,
    })
}

/// 归一化目标路径：反斜杠转 `/`、去掉 `./` 前缀、压掉重复与末尾斜杠。
///
/// **不做大小写归一**：`target` 字段要能原样回显给人看（`MEMORY.md` 不该变成 `memory.md`）。
/// 大小写只在 [`slug_for`] 里折叠，理由见该函数。
#[must_use]
pub fn normalize_target(raw: &str) -> String {
    let unified = raw.trim().replace('\\', "/");
    let without_dot_prefix = unified.strip_prefix("./").unwrap_or(&unified);
    let mut normalized = String::new();
    for character in without_dot_prefix.chars() {
        if character == '/' && (normalized.is_empty() || normalized.ends_with('/')) {
            continue; // 压掉重复斜杠与开头的 `/`
        }
        normalized.push(character);
    }
    while normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

/// 派生锁文件名（不含目录），形如 `MEMORY-md-3f2a9c11.lock`。
///
/// 规则（ADR-0028 D2）：归一化路径 → `/` 换成 `__`、非字母数字换成 `-` →
/// 截断到 [`SLUG_VISIBLE_LIMIT`] 字符 → 拼上 8 位 **FNV-1a** 十六进制摘要。
///
/// **摘要为什么必须有**：① 防重名（`a-b.md` 与 `a/b.md` 折叠后都是 `a-b-md`）；② 防超长
/// （Windows 单个路径段上限 255 字符）。
///
/// **为什么按小写折叠**：Windows 与 macOS 默认文件系统**大小写不敏感**，`MEMORY.md` 与
/// `memory.md` 是同一个文件，分别派生两把锁会让互斥形同虚设。代价是 Linux（大小写敏感）上
/// 会为两个**不同**文件误加同一把锁 —— 对互斥量而言**过度加锁是安全方向**，欠加锁才是事故。
#[must_use]
pub fn slug_for(relative_path: &str) -> String {
    let normalized = normalize_target(relative_path);
    let mut visible = String::new();
    for character in normalized.to_ascii_lowercase().chars() {
        if character == '/' {
            visible.push_str("__");
        } else if character.is_ascii_alphanumeric() {
            visible.push(character);
        } else {
            visible.push('-');
        }
    }
    let truncated: String = visible.chars().take(SLUG_VISIBLE_LIMIT).collect();
    let digest = fnv1a_hex32(&normalized.to_ascii_lowercase());
    format!("{truncated}-{digest}{LOCK_FILE_SUFFIX}")
}

/// FNV-1a 32 位摘要的 8 位十六进制文本。
///
/// 选 FNV-1a 的理由：算法只有 5 行、无查表、零依赖，且**跨平台跨版本完全确定**
/// （`std` 的 `DefaultHasher` 明确不保证稳定，绝不能用来派生文件名）。
#[must_use]
pub fn fnv1a_hex32(text: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in text.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

/// 判定「请求者现在该不该拿到这把锁」。
///
/// 判定顺序是**语义的一部分**，不是随手写的：
/// ① 无锁 → Grant；② 同一 owner → Reuse（自己不该被自己挡住）；
/// ③ `--force` → TakeOver（人工意图优先于一切自动判据）；
/// ④ 获取时刻在未来 → `ClockAnomaly`（**不接管**，见该变体的文档）；
/// ⑤ 锁龄 ≥ 阈值 → TakeOver(Stale)；⑥ 其余 → Wait。
#[must_use]
pub fn decide_acquire(
    existing: Option<&LockRecord>,
    now_unix: u64,
    stale_after_secs: u64,
    owner: &str,
    force: bool,
) -> Acquisition {
    let Some(held) = existing else {
        return Acquisition::Grant;
    };
    if held.owner == owner {
        return Acquisition::Reuse;
    }
    if force {
        return Acquisition::TakeOver {
            held: held.clone(),
            reason: TakeOverReason::Forced,
        };
    }
    if held.acquired_at_unix > now_unix {
        return Acquisition::ClockAnomaly { held: held.clone() };
    }
    if now_unix.saturating_sub(held.acquired_at_unix) >= stale_after_secs {
        return Acquisition::TakeOver {
            held: held.clone(),
            reason: TakeOverReason::Stale,
        };
    }
    Acquisition::Wait { held: held.clone() }
}

/// 锁龄（秒）；获取时刻在未来时返回 0 而不是负数或巨大值。
#[must_use]
pub const fn lock_age_secs(record: &LockRecord, now_unix: u64) -> u64 {
    now_unix.saturating_sub(record.acquired_at_unix)
}

/// 归一化、去重、排序一批目标路径（ADR-0028 D7：死锁避免）。
///
/// **为什么必须排序**：两个 agent 以不同顺序请求同一组文件时（`acquire A B` 与 `acquire B A`），
/// 不排序就会互相等待到死。排序后两者的获取顺序都变成 A → B，等待图不可能成环。
/// 去重同样必要：同一文件请求两次会在第二次上被自己的锁挡住（除非 owner 相同触发 Reuse）。
#[must_use]
pub fn sort_targets(raw_targets: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = raw_targets
        .iter()
        .map(|raw| normalize_target(raw))
        .filter(|path| !path.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

#[cfg(test)]
// 测试体外置到 `guard_tests.rs`（理由见该文件头）；`#[path]` 让它仍是本模块的私有单测。
#[path = "guard_tests.rs"]
mod tests;

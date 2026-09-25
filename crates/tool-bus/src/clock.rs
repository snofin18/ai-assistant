//! 时间来源（AGENTS.md §5.3：**时钟一律 trait 注入**，以便回放可复现）。
//!
//! 为什么不复用 `assistant_storage::Clock`：那个 trait 住在持久化 crate 里，复用它会把
//! `rusqlite` 拖进 tool-bus 的依赖图 —— 而工具通道必须能在**没有数据库**的情况下起来。
//! 两个 trait 的口径**刻意一致**（Unix 毫秒 + 早于 1970 一律取 0），语义重复已记入
//! `docs/PARKING_LOT.md` PL-079（建议将来抽一个 `assistant-time`）。
//!
//! 相关：架构 v2 §7.5（截图 / 证据时间线）、`docs/spec/tool-schema.md` §4 不变量 7（ISO-8601）。

use std::time::{SystemTime, UNIX_EPOCH};

/// 时间来源：唯一被允许的「现在」入口。
///
/// 幂等 / 无副作用：只读系统时钟，不改变任何状态。实现必须 `Send + Sync`
/// （tool-bus 会在多线程运行时里持有它）。
pub trait Clock: Send + Sync {
    /// 当前 Unix 毫秒时间戳。
    fn now_unix_ms(&self) -> i64;

    /// 当前 UTC 时刻的 RFC 3339 字符串（毫秒精度，固定 `Z` 后缀）。
    ///
    /// 提供默认实现（= [`rfc3339_utc_from_unix_ms`]），实现者只需给 `now_unix_ms`。
    fn now_rfc3339(&self) -> String {
        rfc3339_utc_from_unix_ms(self.now_unix_ms())
    }
}

/// 生产用系统时钟。
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_ms(&self) -> i64 {
        // 系统时钟早于 1970-01-01 只可能是机器时间被设错：此时返回 0（一个**明显异常**的
        // 取值），而不是 panic、也不是看似合理的负数。口径与 assistant-storage 的
        // SystemClock 一致（见 crates/storage/src/time_source.rs）。
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
            })
    }
}

/// Unix 毫秒 → RFC 3339（UTC，毫秒精度）。
///
/// 纯函数：同样输入必得同样输出（可回放）。负输入按 0 处理（口径见 [`SystemClock`]）。
#[must_use]
pub fn rfc3339_utc_from_unix_ms(unix_ms: i64) -> String {
    let total_ms = u64::try_from(unix_ms).unwrap_or(0);
    let total_seconds = total_ms / 1_000;
    let millis = total_ms % 1_000;
    let days = total_seconds / 86_400;
    let seconds_of_day = total_seconds % 86_400;

    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// 天数（1970-01-01 = 0）→ `(年, 月, 日)`。
///
/// 算法：Howard Hinnant 的 `civil_from_days` —— 把「400 年 = 146097 天」当作纪元单位，
/// 并把 3 月当作一年的开始（这样闰日落在年末，不需要特判闰年）。
/// 全部用**无符号整数**运算，因此不需要任何 `as` 转换
/// （workspace 把 `clippy::cast_possible_truncation` 当警告，而 `-D warnings` 会把它变红）。
///
/// 示例：`civil_from_days(0)` = `(1970, 1, 1)`；`civil_from_days(19_675)` = `(2023, 11, 14)`。
const fn civil_from_days(days: u64) -> (u64, u64, u64) {
    // 719468 = 从 0000-03-01 到 1970-01-01 的天数。
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400;
    // 1 月 / 2 月在上一步被算成了「上一年的年末」，这里补回来。
    (if month <= 2 { year + 1 } else { year }, month, day)
}

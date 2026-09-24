//! 时钟注入点。
//!
//! 为什么不直接用 `SystemTime::now()`：AGENTS.md §5.3 要求「时钟 / 随机 / UUID / FS / 网络
//! 一律 trait 注入（保证可回放）」。存储层所有时间列都来自 [`Clock`]，测试用固定时钟
//! 就能确定性地验证 TTL / GC / 检查点时间戳。
//!
//! 不变量：`now_unix_ms()` 必须**单调不减**（生产实现用系统时钟，测试实现由调用方保证）。

use std::time::{SystemTime, UNIX_EPOCH};

/// 只读时钟抽象（存储层唯一的"当前时间"来源）。
pub trait Clock: Send + Sync {
    /// 当前 Unix 毫秒时间戳。
    fn now_unix_ms(&self) -> i64;
}

/// 生产用系统时钟。
///
/// 边界：只提供"现在"，不提供定时器 / 时区 / 格式化（那些不属于存储层）。
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_ms(&self) -> i64 {
        // 系统时钟早于 1970-01-01 只可能是机器时间被设错；此时返回 0（而不是 panic 或负数），
        // 让调用方看到"时间戳为 0"这个明显的异常值，而不是一个看似合理的数字。
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
            })
    }
}

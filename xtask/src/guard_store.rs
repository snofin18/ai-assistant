//! # guard 的外部能力层：锁存储（ADR-0028 D2/D3 的落地）
//!
//! 职责：把「锁放在哪、叫什么名字、怎么原子创建、怎么读回来、怎么记放弃日志、现在几点」
//! 这些**与判定无关的外部能力**收在一个 trait 里，并给出生产实现 [`FileLockStore`]。
//!
//! ## 为什么要抽 trait（而不是直接用 `std::fs`）
//! guard 最关键的一条路径是「等待 → 超时 → 放弃 → 通报」，它**同时依赖真实时间与文件系统**。
//! 直接写死就只有两种测法：真的睡 30 秒（CI 不可接受），或者不测（违反 ADR-0019 的元门禁要求：
//! 每道防线都得证明它在该红的时候真的会红）。抽成 trait 后，测试用内存替身就能覆盖全部分支，
//! 包括「时钟回拨」这种在真机上几乎无法构造的情况。这也是 `AGENTS.md` §5.3
//! 「时钟/随机/UUID/FS/网络一律 trait 注入（保证可回放）」的落实。
//!
//! ## 边界（不做什么）
//! - 不做判定：该不该拿到锁在 `guard.rs::decide_acquire`。
//! - 不解析命令行、不渲染报告：那是 `guard_runner.rs` / `guard_release.rs` 的事。
//! - 不碰仓库内容：唯一写入的是 `target/locks/` 下的锁文件与放弃日志，
//!   两者都被 `.gitignore` 的 `/target/` 覆盖，**不入库**（ADR-0028 D8 的受限放宽）。
//!
//! ## 不变量
//! 1. `create` 必须是**原子的「不存在才创建」**（ADR-0028 D3：POSIX 的 `O_CREAT|O_EXCL`、
//!    Windows 的 `CREATE_NEW`）。这是整套机制的唯一并发原语，退化成立即失效。
//! 2. 「文件不存在」与「读不出来」必须区分：前者返回 `Ok(None)`，后者返回 `Err`。
//!    把读失败当成不存在，会让两个进程同时认为自己拿到了锁（铁律 1）。
//! 3. 路径派生（[`locks_directory`] / [`lock_file_path`]）是纯函数，跨平台一致。
//!
//! 相关：`docs/adr/0028-file-rewrite-mutex-protocol.md`、`xtask/src/guard.rs`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::guard::{ABANDONMENT_LOG, LOCK_DIR, LOCK_FILE_SUFFIX, slug_for};

/// guard 需要的全部外部能力。
///
/// 方法以**目标路径**（仓库相对路径）为单位，而不是以 `Path` 为单位：
/// 这样调用方不需要知道锁目录在哪，替身实现也不必模拟目录结构。
pub trait LockStore {
    /// 当前 Unix 秒（陈旧判定与锁龄的唯一时间来源）。
    fn now_unix(&self) -> u64;
    /// 当前时刻的 ISO-8601 UTC 文本（给人看的诊断字段）。
    fn now_iso(&self) -> String;
    /// 当前进程 id（仅诊断线索，**不参与存活判定** —— ADR-0028 D6）。
    fn current_pid(&self) -> u32;
    /// 确保锁目录存在。
    ///
    /// # Errors
    /// 创建失败时返回原因。
    fn prepare(&self) -> Result<(), String>;
    /// 读某个目标的锁记录文本；不存在返回 `None`。
    ///
    /// # Errors
    /// 文件存在但读不出来时返回原因（不变量 2）。
    fn read(&self, target: &str) -> Result<Option<String>, String>;
    /// 原子创建某个目标的锁并写入内容。
    ///
    /// # Errors
    /// 锁已存在（被别人抢先）或写入失败时返回原因。
    fn create(&self, target: &str, content: &str) -> Result<(), String>;
    /// 按目标路径删除锁。
    ///
    /// # Errors
    /// 删除失败时返回原因（Windows 上文件被占用会走到这里，**不静默**）。
    fn remove(&self, target: &str) -> Result<(), String>;
    /// 按锁**文件名**删除（`reap` 只有文件名，损坏的锁记录里没有 target 可用）。
    ///
    /// # Errors
    /// 删除失败时返回原因。
    fn remove_by_file_name(&self, lock_file_name: &str) -> Result<(), String>;
    /// 往放弃日志追加一行。
    ///
    /// # Errors
    /// 写入失败时返回原因。
    fn append_log(&self, line: &str) -> Result<(), String>;
    /// 列出锁目录里全部 `.lock` 文件（文件名 → 内容），按文件名排序（输出确定性）。
    ///
    /// # Errors
    /// 目录不可读时返回原因；目录**不存在**返回空表而不是错误。
    fn list(&self) -> Result<BTreeMap<String, String>, String>;
    /// 睡眠若干毫秒（轮询退避用）。
    fn sleep(&self, millis: u64);
}

/// 锁目录（相对仓库根解析成绝对路径）。
#[must_use]
pub fn locks_directory(repo_root: &Path) -> PathBuf {
    join_relative(repo_root, LOCK_DIR)
}

/// 放弃日志（相对仓库根解析成绝对路径）。
#[must_use]
pub fn abandonment_log(repo_root: &Path) -> PathBuf {
    join_relative(repo_root, ABANDONMENT_LOG)
}

/// 单个目标的锁文件路径。
#[must_use]
pub fn lock_file_path(repo_root: &Path, target: &str) -> PathBuf {
    locks_directory(repo_root).join(slug_for(target))
}

/// 把一个统一用 `/` 写的仓库相对路径接到根上（跨平台正确的分隔符）。
fn join_relative(repo_root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(repo_root.to_path_buf(), |accumulated, segment| {
            accumulated.join(segment)
        })
}

/// 把 Unix 秒格式化成 ISO-8601 UTC 文本（`YYYY-MM-DDThh:mm:ssZ`）。
///
/// 刻意自己算而不用 `chrono`：xtask 零第三方依赖（`xtask/Cargo.toml` 不变量 1）。
/// 算法是 Howard Hinnant 的 `civil_from_days`（公历格里高利历，对 1970 前后都正确），
/// 全程用 `i64` 运算，**没有任何类型转换**，因此不会触发 `clippy::cast_*` 一族告警。
#[must_use]
pub fn iso8601_utc(unix_secs: u64) -> String {
    // u64 → i64：Unix 秒在可预见的未来远小于 i64::MAX，饱和即可（不 panic）
    let seconds = i64::try_from(unix_secs).unwrap_or(i64::MAX);
    let days = seconds.div_euclid(86_400);
    let remainder = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = remainder / 3_600;
    let minute = (remainder % 3_600) / 60;
    let second = remainder % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// 把「1970-01-01 起的天数」换算成公历 (年, 月, 日)。
///
/// 算法要点：以 400 年（146097 天）为一个"纪元"，先在纪元内算出 year-of-era 与 day-of-year，
/// 再把 3 月起始的月份编号换算回 1 月起始。全部用整数除法，无浮点、无查表。
const fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let shifted = days_since_epoch + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097); // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153; // [0, 11]，0 = 三月
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1; // [1, 31]
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// 生产环境的锁存储（真实时钟 + 真实文件系统 + 真实睡眠）。
#[derive(Debug, Clone)]
pub struct FileLockStore {
    /// 仓库根（锁目录与放弃日志都相对它解析）。
    repo_root: PathBuf,
}

impl FileLockStore {
    /// 以仓库根构造一个锁存储。
    #[must_use]
    pub const fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    /// 某个目标的锁文件路径。
    #[must_use]
    pub fn path_for(&self, target: &str) -> PathBuf {
        lock_file_path(&self.repo_root, target)
    }
}

impl LockStore for FileLockStore {
    fn now_unix(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            // 系统时钟早于 1970 时无处可报：返回 0 会让所有锁看起来"极其陈旧"从而被集体接管，
            // 但接管是**可见的**（会打印被接管者的完整记录 + 写放弃日志），比 panic 或静默好
            .map_or(0, |elapsed| elapsed.as_secs())
    }

    fn now_iso(&self) -> String {
        iso8601_utc(self.now_unix())
    }

    fn current_pid(&self) -> u32 {
        std::process::id()
    }

    fn prepare(&self) -> Result<(), String> {
        let directory = locks_directory(&self.repo_root);
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("创建 {} 失败：{error}", directory.display()))
    }

    fn read(&self, target: &str) -> Result<Option<String>, String> {
        let path = self.path_for(target);
        match std::fs::read_to_string(&path) {
            Ok(content) => Ok(Some(content)),
            // 不变量 2：只有"确实不存在"才是 None，其它 IO 错误一律上抛
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("读取 {} 失败：{error}", path.display())),
        }
    }

    fn create(&self, target: &str, content: &str) -> Result<(), String> {
        use std::fs::OpenOptions;
        use std::io::Write as _;
        let path = self.path_for(target);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true) // 不变量 1：原子的"不存在才创建"
            .open(&path)
            .map_err(|error| format!("创建 {} 失败：{error}", path.display()))?;
        file.write_all(content.as_bytes())
            .map_err(|error| format!("写入 {} 失败：{error}", path.display()))
    }

    fn remove(&self, target: &str) -> Result<(), String> {
        let path = self.path_for(target);
        remove_file_quietly_if_absent(&path)
    }

    fn remove_by_file_name(&self, lock_file_name: &str) -> Result<(), String> {
        let path = locks_directory(&self.repo_root).join(lock_file_name);
        remove_file_quietly_if_absent(&path)
    }

    fn append_log(&self, line: &str) -> Result<(), String> {
        use std::fs::OpenOptions;
        use std::io::Write as _;
        let path = abandonment_log(&self.repo_root);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| format!("打开 {} 失败：{error}", path.display()))?;
        file.write_all(format!("{line}\n").as_bytes())
            .map_err(|error| format!("写入 {} 失败：{error}", path.display()))
    }

    fn list(&self) -> Result<BTreeMap<String, String>, String> {
        let directory = locks_directory(&self.repo_root);
        let mut found = BTreeMap::new();
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(found),
            Err(error) => return Err(format!("列目录 {} 失败：{error}", directory.display())),
        };
        for entry in entries {
            let entry = entry
                .map_err(|error| format!("读取 {} 的目录项失败：{error}", directory.display()))?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.to_ascii_lowercase().ends_with(LOCK_FILE_SUFFIX) {
                continue;
            }
            let content = std::fs::read_to_string(entry.path())
                .map_err(|error| format!("读取 {file_name} 失败：{error}"))?;
            found.insert(file_name, content);
        }
        Ok(found)
    }

    fn sleep(&self, millis: u64) {
        std::thread::sleep(std::time::Duration::from_millis(millis));
    }
}

/// 删除文件；**文件本来就不存在视为成功**（目的状态已成立），其它错误上抛。
///
/// 为什么容忍"不存在"：`reap` 与 `release` 都可能与别人的 `release` 竞争，
/// 对方先删掉了 → 我们要的结果（没有这把锁）已经达成，报错误导人。
fn remove_file_quietly_if_absent(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("删除 {} 失败：{error}", path.display())),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_iso8601_utc_matches_known_timestamps() {
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601_utc(86_399), "1970-01-01T23:59:59Z");
        assert_eq!(iso8601_utc(86_400), "1970-01-02T00:00:00Z");
        // 2026-09-18T00:00:00Z = 1789689600（用 DateTimeOffset.ToUnixTimeSeconds 核对过）
        assert_eq!(iso8601_utc(1_789_689_600), "2026-09-18T00:00:00Z");
    }

    #[test]
    fn test_iso8601_utc_handles_leap_day() {
        // 2024-02-29T12:34:56Z = 1709210096
        assert_eq!(iso8601_utc(1_709_210_096), "2024-02-29T12:34:56Z");
    }

    #[test]
    fn test_civil_from_days_handles_pre_epoch_dates() {
        // 1969-12-31 = 第 -1 天
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        // 1900-01-01 = 第 -25567 天
        assert_eq!(civil_from_days(-25_567), (1900, 1, 1));
    }

    #[test]
    fn test_locks_directory_and_log_path_are_under_target() {
        let root = Path::new("/repo");
        assert_eq!(
            locks_directory(root).to_string_lossy().replace('\\', "/"),
            "/repo/target/locks"
        );
        assert_eq!(
            abandonment_log(root).to_string_lossy().replace('\\', "/"),
            "/repo/target/locks/abandonments.log"
        );
    }

    #[test]
    fn test_lock_file_path_uses_slug_and_stays_in_lock_directory() {
        let path = lock_file_path(Path::new("/repo"), "docs/memory/facts.md");
        let text = path.to_string_lossy().replace('\\', "/");
        assert!(text.starts_with("/repo/target/locks/"), "实际：{text}");
        assert!(
            text.ends_with(&slug_for("docs/memory/facts.md")),
            "实际：{text}"
        );
    }

    #[test]
    fn test_join_relative_handles_single_segment() {
        assert_eq!(
            join_relative(Path::new("/repo"), "MEMORY.md")
                .to_string_lossy()
                .replace('\\', "/"),
            "/repo/MEMORY.md"
        );
    }

    #[test]
    fn test_file_lock_store_path_for_is_stable_for_same_target() {
        let store = FileLockStore::new(PathBuf::from("/repo"));
        assert_eq!(store.path_for("MEMORY.md"), store.path_for("MEMORY.md"));
        // 不变量 3 的实际意义：大小写不同的写法必须落到同一把锁（Windows/macOS 同一文件）
        assert_eq!(store.path_for("MEMORY.md"), store.path_for("memory.md"));
    }
}

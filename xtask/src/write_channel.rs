//! # 命令行唯一写通道：guard 锁 + 目标占用探测（ADR-0066）
//!
//! 职责：给 `xtask write` 提供一条**可诊断、可超时、读取不受阻**的写入路径。
//! 它先把 ADR-0028 的协作式 `guard` 锁套在目标上，再在 Windows 上探测目标是否已被
//! 其它进程以不共享的方式占用；占用时退避重试，超时则退出码 5 并写放弃日志。
//!
//! ## 边界（不做什么）
//! - **不取代 `guard`**：没有常驻 FIFO、没有公平队列；这里只有互斥 + 有界退避。
//! - **不承诺强制互斥**：`FILE_SHARE_WRITE` 的持有者对独占探测不可见，ADR-0066 已明写。
//! - **不接管 `apply_patch`**：它由编辑器工具执行，本通道只覆盖命令行协作写者。
//! - 不启动子进程，因此没有需要 kill / reap 的进程树；stdin 读取有 16 MiB 硬上限。
//!
//! ## 不变量
//! 1. 写入前必须已经成功取得 guard 锁；取得失败或目标占用超时都必须返回退出码 5，
//!    不得把“没写成”伪装成成功。
//! 2. 目标路径必须是仓库内相对路径；绝对路径与 `..` 段一律用法错误。
//! 3. 一旦成功取锁，成功、失败、超时三条路径都必须尝试释放锁；释放失败必须显式报错。
//! 4. 实际写句柄只允许读取者共享（Windows `FILE_SHARE_READ`），读取路径不进入通道。
//! 5. 非 Windows 环境独占探测不可用时 fail-closed，不静默降级为“无探测写入”。
//!
//! 相关：`docs/adr/0066-cli-single-write-channel-and-file-occupancy-probe.md`、
//! `docs/adr/0028-file-rewrite-mutex-protocol.md`、`xtask/README.md`

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::cli::Invocation;
use crate::guard::POLL_INTERVAL_MILLIS;
use crate::guard_model::{GuardFailure, GuardRequest, build_request, write_line};
use crate::guard_store::LockStore;
use crate::{EXIT_LOCK_TIMEOUT, EXIT_OK};

/// `xtask write` 的默认超时（秒）。
///
/// 比 `guard` 的 30 秒更短：写入等待必须尽快把控制权还给 PowerShell 会话，
/// 避免再次触发“命令看起来永远没返回”的故障形态。调用方可用 `--timeout` 覆盖。
pub const DEFAULT_WRITE_TIMEOUT_SECS: u64 = 5;

/// stdin 允许读入的最大字节数。
///
/// 本工具不是编辑器；16 MiB 足够覆盖仓库里的文本文件，同时满足 ADR-0063
/// “长期状态必须有硬上限”的纪律。超过上限必须失败，不得无界读入内存。
pub const MAX_WRITE_BYTES: usize = 16 * 1024 * 1024;

/// 一次目标占用探测的结果。
///
/// 刻意用结构体而不是带平台条件变体的枚举：非 Windows 目标只实现 fail-closed 探测，
/// 枚举分支会在 `clippy -D warnings` 下被 `dead_code` 判死。这里的字段在
/// `wait_for_free` 中始终会被读取，跨平台构建不会产生未使用分支。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutcome {
    /// 目标是否被占用；`false` 表示可继续写入。
    pub is_busy: bool,
    /// 被占用时的可诊断原因；空闲时为空串。
    pub reason: String,
}

/// 目标文件的平台访问边界。
///
/// 把探测与写入抽成 trait 的理由：Windows sharing mode 的真实行为可以在集成式测试里
/// 单独验证，而超时、锁释放、输入上限等分支用内存替身就能穷尽覆盖。
pub trait TargetAccess {
    /// 独占探测目标；不修改目标内容。
    ///
    /// # Errors
    /// 除“被占用”与“平台不支持”之外的 IO 错误原样返回。
    fn probe_exclusive(&self, path: &Path) -> Result<ProbeOutcome, String>;

    /// 在已经通过探测后写入目标。
    ///
    /// # Errors
    /// 打开、写入或刷新失败时返回原因。
    fn write_target(&self, path: &Path, bytes: &[u8]) -> Result<(), String>;
}

/// 真实文件系统实现。
#[derive(Debug, Default, Clone, Copy)]
pub struct FileSystemTargetAccess;

impl TargetAccess for FileSystemTargetAccess {
    fn probe_exclusive(&self, path: &Path) -> Result<ProbeOutcome, String> {
        platform_probe_exclusive(path)
    }

    fn write_target(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        platform_write_target(path, bytes)
    }
}

/// 从已解析的命令行调用执行 `write`，stdin 由本函数接管。
///
/// # Errors
/// 目标数不为 1、stdin 超限、guard 失败、平台不支持或写入失败时返回 [`GuardFailure`]。
pub fn run_from_invocation(
    invocation: &Invocation,
    repo_root: &Path,
    store: &dyn LockStore,
    access: &dyn TargetAccess,
    output: &mut dyn Write,
) -> Result<u8, GuardFailure> {
    if invocation.operands.len() != 1 {
        return Err(GuardFailure::Usage(
            "write 需要一个且仅一个仓库相对路径；内容从 stdin 读入".to_string(),
        ));
    }
    let mut options = invocation.options.clone();
    // 只有调用方没有显式给 `--timeout` 时才使用 write 专用的短默认值。
    options
        .entry("timeout".to_string())
        .or_insert_with(|| DEFAULT_WRITE_TIMEOUT_SECS.to_string());
    let request = build_request(
        Some("acquire"),
        &invocation.operands,
        &options,
        invocation.has_flag("force"),
    )?;
    let mut source = std::io::stdin().lock();
    run(&request, repo_root, store, access, &mut source, output)
}

/// 执行一次 `xtask write`。
///
/// # Errors
/// 参数无效、stdin 超限、guard 失败、平台不支持、写入失败或锁释放失败时返回
/// [`GuardFailure`]；目标占用超时返回 `Ok(EXIT_LOCK_TIMEOUT)`。
pub fn run(
    request: &GuardRequest,
    repo_root: &Path,
    store: &dyn LockStore,
    access: &dyn TargetAccess,
    source: &mut dyn Read,
    output: &mut dyn Write,
) -> Result<u8, GuardFailure> {
    let target = single_target(request)?;
    let target_path = resolve_target_path(repo_root, target)?;
    let bytes = read_bounded(source)?;

    // 先读入内容再取锁：stdin 可能长时间阻塞，不能把 guard 锁白白占住。
    let acquire_code = crate::guard_runner::run_acquire(request, store, output)?;
    if acquire_code != EXIT_OK {
        return Ok(acquire_code);
    }

    let wait = match wait_for_free(access, store, &target_path, request.timeout_secs) {
        Ok(wait) => wait,
        Err(failure) => {
            release_lock(request, store, output)?;
            return Err(failure);
        }
    };
    match wait {
        ProbeWait::Free => {}
        ProbeWait::TimedOut {
            waited_millis,
            reason,
        } => {
            release_lock(request, store, output)?;
            let message = format!("target={target} occupied={reason} waited_ms={waited_millis}");
            store.append_log(&format!(
                "WRITE_ABANDONED owner={} {message}",
                request.owner
            ))?;
            write_line(output, &format!("-- write-result: ABANDONED {message}"))?;
            write_line(
                output,
                "通报义务（ADR-0028 D5 ④ / ADR-0066 D7）：请在 LEDGER.md 追加一行，说明放弃了什么、为什么。",
            )?;
            return Ok(EXIT_LOCK_TIMEOUT);
        }
    }

    if let Err(reason) = access.write_target(&target_path, &bytes) {
        release_lock(request, store, output)?;
        return Err(GuardFailure::Io(format!("写入 {target} 失败：{reason}")));
    }

    release_lock(request, store, output)?;
    write_line(
        output,
        &format!(
            "-- write-result: WRITTEN target={target} bytes={}",
            bytes.len()
        ),
    )?;
    Ok(EXIT_OK)
}

/// 校验请求里只有一个目标，并返回其文本。
fn single_target(request: &GuardRequest) -> Result<&str, GuardFailure> {
    match request.targets.as_slice() {
        [target] if !target.is_empty() => Ok(target),
        [] => Err(GuardFailure::Usage(
            "write 至少需要一个目标路径，例如 `xtask write LEDGER.md`".to_string(),
        )),
        _ => Err(GuardFailure::Usage(
            "write 一次只能写一个目标；多个目标请分开调用，避免部分成功难以诊断".to_string(),
        )),
    }
}

/// 把仓库相对路径解析成绝对路径，并拒绝绝对路径、空路径与 `..` 穿越。
fn resolve_target_path(repo_root: &Path, target: &str) -> Result<PathBuf, GuardFailure> {
    let raw_path = Path::new(target);
    if raw_path.is_absolute() {
        return Err(GuardFailure::Usage(format!(
            "write 只接受仓库相对路径，不接受绝对路径 `{target}`"
        )));
    }
    let normalized = crate::guard::normalize_target(target);
    if normalized.is_empty()
        || normalized
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(GuardFailure::Usage(format!(
            "write 目标必须是仓库内相对路径，实际是 `{target}`"
        )));
    }
    Ok(repo_root.join(normalized))
}

/// 从 stdin 读入有界字节；超限即用法错误，避免把一大段输入静默截断。
fn read_bounded(source: &mut dyn Read) -> Result<Vec<u8>, GuardFailure> {
    let limit = u64::try_from(MAX_WRITE_BYTES).unwrap_or(u64::MAX);
    let mut bytes = Vec::new();
    source
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| GuardFailure::Io(format!("读取 stdin 失败：{error}")))?;
    if bytes.len() > MAX_WRITE_BYTES {
        return Err(GuardFailure::Usage(format!(
            "stdin 超过 {MAX_WRITE_BYTES} 字节硬上限；write 不接受无界输入"
        )));
    }
    Ok(bytes)
}

/// 目标占用等待的结果。
enum ProbeWait {
    /// 目标已空闲。
    Free,
    /// 在超时前一直忙。
    TimedOut {
        /// 实际等待毫秒数。
        waited_millis: u64,
        /// 最后一次探测看到的忙原因。
        reason: String,
    },
}

/// 以固定退避轮询目标占用，直到空闲或超时。
fn wait_for_free(
    access: &dyn TargetAccess,
    store: &dyn LockStore,
    path: &Path,
    timeout_secs: u64,
) -> Result<ProbeWait, GuardFailure> {
    let started_at = store.now_unix();
    let timeout_millis = timeout_secs.saturating_mul(1000);
    loop {
        let outcome = access.probe_exclusive(path)?;
        if !outcome.is_busy {
            return Ok(ProbeWait::Free);
        }
        let waited_millis = store
            .now_unix()
            .saturating_sub(started_at)
            .saturating_mul(1000);
        if waited_millis >= timeout_millis {
            return Ok(ProbeWait::TimedOut {
                waited_millis,
                reason: outcome.reason,
            });
        }
        store.sleep(POLL_INTERVAL_MILLIS);
    }
}

/// 释放 `run_acquire` 刚刚取得的锁；非零退出码视为显式失败。
fn release_lock(
    request: &GuardRequest,
    store: &dyn LockStore,
    output: &mut dyn Write,
) -> Result<(), GuardFailure> {
    let code = crate::guard_release::run_release(request, store, output)?;
    if code != EXIT_OK {
        return Err(GuardFailure::Io(format!(
            "write 释放 guard 锁返回退出码 {code}；锁可能仍在，需人工检查"
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn platform_probe_exclusive(path: &Path) -> Result<ProbeOutcome, String> {
    use std::os::windows::fs::OpenOptionsExt;

    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;

    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)
    {
        Ok(_) => Ok(ProbeOutcome {
            is_busy: false,
            reason: String::new(),
        }),
        Err(error) if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION) => Ok(ProbeOutcome {
            is_busy: true,
            reason: format!("sharing_violation(raw_os_error={ERROR_SHARING_VIOLATION})"),
        }),
        Err(error) if error.raw_os_error() == Some(ERROR_LOCK_VIOLATION) => Ok(ProbeOutcome {
            is_busy: true,
            reason: format!("lock_violation(raw_os_error={ERROR_LOCK_VIOLATION})"),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ProbeOutcome {
            is_busy: false,
            reason: String::new(),
        }),
        Err(error) => Err(format!("独占探测 {} 失败：{error}", path.display())),
    }
}

#[cfg(not(windows))]
fn platform_probe_exclusive(_path: &Path) -> Result<ProbeOutcome, String> {
    Err("独占占用探测仅支持 Windows；当前平台请继续使用 `guard acquire` 的协作式锁".to_string())
}

#[cfg(windows)]
fn platform_write_target(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = open_target_for_write(path)?;
    file.write_all(bytes)
        .map_err(|error| format!("写入 {} 失败：{error}", path.display()))?;
    file.flush()
        .map_err(|error| format!("刷新 {} 失败：{error}", path.display()))
}

#[cfg(windows)]
fn open_target_for_write(path: &Path) -> Result<std::fs::File, String> {
    use std::os::windows::fs::OpenOptionsExt;

    // 只共享读取：读者可以打开并读取，其它写者会被 Windows 拒绝。
    const FILE_SHARE_READ: u32 = 0x0000_0001;

    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)
        .map_err(|error| format!("打开 {} 失败：{error}", path.display()))
}

#[cfg(not(windows))]
fn platform_write_target(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|error| format!("打开 {} 失败：{error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("写入 {} 失败：{error}", path.display()))?;
    file.flush()
        .map_err(|error| format!("刷新 {} 失败：{error}", path.display()))
}

#[cfg(test)]
#[path = "write_channel_tests.rs"]
mod tests;

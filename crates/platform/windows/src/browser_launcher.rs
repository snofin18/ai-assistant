//! Launches a browser with one dedicated profile and reclaims its process tree.
//!
//! Responsibilities:
//! - validate the browser executable and dedicated profile root;
//! - build a fixed launch command with a dynamic CDP port;
//! - read and validate the browser-written `DevToolsActivePort` file;
//! - terminate the complete process tree and reap the root process.
//!
//! Boundaries:
//! - does not start a browser during tests or select a production browser path;
//! - does not copy or read a user profile, cookies, credentials, or login state;
//! - does not register production tool handlers or expose browser OS handles.

use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::os::windows::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use assistant_browser_profile::{BrowserKind, BrowserProfile};
use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};

const ACTIVE_PORT_FILE_NAME: &str = "DevToolsActivePort";
const MAX_ACTIVE_PORT_BYTES: usize = 4 * 1024;
const MAX_BROWSER_PATH_BYTES: usize = 512;
const MIN_LAUNCH_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_LAUNCH_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);
const TERMINATE_TIMEOUT: Duration = Duration::from_secs(10);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const WINDOWS_ERROR_SHARING_VIOLATION: i32 = 32;
const WINDOWS_ERROR_LOCK_VIOLATION: i32 = 33;

/// Configuration for one dedicated browser launch.
#[derive(Debug, Clone)]
pub struct BrowserLauncher {
    executable: PathBuf,
    profile_root: PathBuf,
    profile: BrowserProfile,
    launch_timeout: Duration,
}

impl BrowserLauncher {
    /// Creates a launcher rooted at an explicit dedicated-profile directory.
    ///
    /// # Errors
    ///
    /// Returns `ToolInvalidArgs` for a relative profile root, an invalid profile
    /// id, a profile root under a known browser user-data tree, or an executable
    /// that does not match `browser_kind`. Returns `CapabilityMissing` when the
    /// executable does not exist.
    pub fn new(
        executable: impl AsRef<Path>,
        profile_root: impl AsRef<Path>,
        profile_id: impl Into<String>,
        browser_kind: BrowserKind,
    ) -> PlatformResult<Self> {
        let profile_root = profile_root.as_ref();
        if !profile_root.is_absolute() {
            return Err(invalid_args("browser profile root must be absolute"));
        }
        reject_user_profile_tree(profile_root)?;

        let profile_id = profile_id.into();
        let profile = BrowserProfile::new(
            profile_id.clone(),
            browser_kind,
            profile_root.join(&profile_id),
        )
        .map_err(|error| invalid_args(format!("invalid browser profile: {error}")))?;

        let executable = fs::canonicalize(executable).map_err(|error| {
            PlatformError::new(
                ErrorCode::CapabilityMissing,
                format!("browser executable is unavailable: {error}"),
            )
        })?;
        if !executable.is_file() {
            return Err(PlatformError::new(
                ErrorCode::CapabilityMissing,
                "browser executable path is not a file",
            ));
        }
        validate_browser_executable(&executable, browser_kind)?;

        Ok(Self {
            executable,
            profile_root: profile_root.to_path_buf(),
            profile,
            launch_timeout: DEFAULT_LAUNCH_TIMEOUT,
        })
    }

    /// Overrides the bounded wait for browser CDP readiness.
    ///
    /// # Errors
    ///
    /// Values outside 1..=60 seconds return `ToolInvalidArgs`.
    pub fn with_launch_timeout(mut self, timeout: Duration) -> PlatformResult<Self> {
        if !(MIN_LAUNCH_TIMEOUT..=MAX_LAUNCH_TIMEOUT).contains(&timeout) {
            return Err(invalid_args(
                "browser launch timeout must be between 1 and 60 seconds",
            ));
        }
        self.launch_timeout = timeout;
        Ok(self)
    }

    /// Preserves the profile's first-login marker across this launch.
    #[must_use]
    pub const fn require_first_login(mut self, required: bool) -> Self {
        self.profile.set_first_login_required(required);
        self
    }

    /// Starts the browser and waits for its loopback CDP endpoint metadata.
    ///
    /// No real browser is started by the crate tests. A launch failure attempts
    /// tree termination before returning the original error.
    ///
    /// # Errors
    ///
    /// Returns `ToolInvalidArgs` for an unsafe profile path or malformed
    /// `DevToolsActivePort`; `CapabilityMissing` when process creation fails;
    /// `TargetUnresponsive` for early exit or launch timeout.
    pub fn launch(self) -> PlatformResult<LaunchedBrowser> {
        let profile = self.prepare_profile()?;
        remove_stale_active_port(profile.user_data_dir())?;
        let arguments = fixed_launch_arguments(&profile);
        let mut child = Command::new(&self.executable)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map_err(|error| {
                PlatformError::new(
                    ErrorCode::CapabilityMissing,
                    format!("failed to start dedicated browser: {error}"),
                )
            })?;

        match wait_for_active_port(&mut child, profile.user_data_dir(), self.launch_timeout) {
            Ok(endpoint) => Ok(LaunchedBrowser {
                child,
                profile,
                endpoint,
            }),
            Err(start_error) => {
                let cleanup = terminate_child_tree(&mut child, TERMINATE_TIMEOUT);
                Err(with_cleanup_context(start_error, cleanup))
            }
        }
    }

    fn prepare_profile(&self) -> PlatformResult<BrowserProfile> {
        fs::create_dir_all(&self.profile_root).map_err(|error| {
            PlatformError::new(
                ErrorCode::Fatal,
                format!("failed to create dedicated profile root: {error}"),
            )
        })?;
        let canonical_root = fs::canonicalize(&self.profile_root).map_err(|error| {
            PlatformError::new(
                ErrorCode::Fatal,
                format!("failed to resolve dedicated profile root: {error}"),
            )
        })?;
        reject_user_profile_tree(&canonical_root)?;

        let profile_dir = canonical_root.join(self.profile.profile_id());
        fs::create_dir_all(&profile_dir).map_err(|error| {
            PlatformError::new(
                ErrorCode::Fatal,
                format!("failed to create dedicated browser profile: {error}"),
            )
        })?;
        let canonical_profile = fs::canonicalize(&profile_dir).map_err(|error| {
            PlatformError::new(
                ErrorCode::Fatal,
                format!("failed to resolve dedicated browser profile: {error}"),
            )
        })?;
        if !canonical_profile.starts_with(&canonical_root) {
            return Err(invalid_args(
                "browser profile escaped its dedicated profile root",
            ));
        }
        reject_user_profile_tree(&canonical_profile)?;

        let mut profile = BrowserProfile::new(
            self.profile.profile_id(),
            self.profile.browser_kind(),
            canonical_profile,
        )
        .map_err(|error| invalid_args(format!("invalid browser profile: {error}")))?;
        profile.set_first_login_required(self.profile.is_first_login_required());
        Ok(profile)
    }
}

/// One launched browser whose root process remains owned by this value.
#[derive(Debug)]
pub struct LaunchedBrowser {
    child: Child,
    profile: BrowserProfile,
    endpoint: ActiveDevToolsPort,
}

impl LaunchedBrowser {
    /// Dedicated profile used by this browser process.
    #[must_use]
    pub const fn profile(&self) -> &BrowserProfile {
        &self.profile
    }

    /// Dynamic loopback CDP port selected by the browser.
    #[must_use]
    pub const fn cdp_port(&self) -> u16 {
        self.endpoint.port
    }

    /// Browser-level `DevTools` WebSocket path.
    #[must_use]
    pub fn browser_websocket_path(&self) -> &str {
        &self.endpoint.browser_path
    }

    /// Loopback browser-level WebSocket URL.
    #[must_use]
    pub fn browser_websocket_url(&self) -> String {
        format!(
            "ws://127.0.0.1:{}{}",
            self.endpoint.port, self.endpoint.browser_path
        )
    }

    /// Terminates the browser process tree and waits until the root is reaped.
    ///
    /// # Errors
    ///
    /// Returns an explicit error if a still-running tree cannot be terminated or
    /// the root process cannot be reaped within the bounded wait.
    pub fn close(&mut self) -> PlatformResult<()> {
        terminate_child_tree(&mut self.child, TERMINATE_TIMEOUT)
    }
}

impl Drop for LaunchedBrowser {
    fn drop(&mut self) {
        terminate_child_tree_best_effort(&mut self.child);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveDevToolsPort {
    port: u16,
    browser_path: String,
}

fn fixed_launch_arguments(profile: &BrowserProfile) -> Vec<String> {
    let mut arguments = profile.launch_args().to_vec();
    arguments.extend([
        "--remote-debugging-port=0".to_string(),
        "--new-window".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-extensions".to_string(),
        "--disable-sync".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-component-update".to_string(),
        "--disable-features=OptimizationHints,MediaRouter".to_string(),
        "--force-renderer-accessibility".to_string(),
        "about:blank".to_string(),
    ]);
    arguments
}

fn validate_browser_executable(path: &Path, browser_kind: BrowserKind) -> PlatformResult<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid_args("browser executable has no UTF-8 file name"))?;
    let expected = match browser_kind {
        BrowserKind::Edge => "msedge.exe",
        BrowserKind::Chrome => "chrome.exe",
        _ => {
            return Err(PlatformError::new(
                ErrorCode::CapabilityMissing,
                "requested browser kind is not supported",
            ));
        }
    };
    if !file_name.eq_ignore_ascii_case(expected) {
        return Err(invalid_args(format!(
            "browser executable must be named {expected}"
        )));
    }
    Ok(())
}

fn reject_user_profile_tree(path: &Path) -> PlatformResult<()> {
    for user_data_root in known_user_data_roots() {
        if path_has_prefix(path, &user_data_root) {
            return Err(invalid_args(
                "dedicated browser profile must not be under a user browser profile",
            ));
        }
    }
    Ok(())
}

fn known_user_data_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for environment_key in ["LOCALAPPDATA", "APPDATA"] {
        let Some(base) = std::env::var_os(environment_key).map(PathBuf::from) else {
            continue;
        };
        roots.push(base.join("Microsoft").join("Edge").join("User Data"));
        roots.push(base.join("Google").join("Chrome").join("User Data"));
    }
    roots
        .into_iter()
        .map(|root| fs::canonicalize(&root).unwrap_or(root))
        .collect()
}

fn path_has_prefix(path: &Path, prefix: &Path) -> bool {
    let path_components = comparable_components(path);
    let prefix_components = comparable_components(prefix);
    !prefix_components.is_empty()
        && prefix_components.len() <= path_components.len()
        && prefix_components
            .iter()
            .zip(&path_components)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn comparable_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Prefix(prefix) => Some(
                prefix
                    .as_os_str()
                    .to_string_lossy()
                    .trim_start_matches(r"\\?\")
                    .to_string(),
            ),
            Component::RootDir => Some("/".to_string()),
            Component::CurDir => None,
            Component::ParentDir => Some("..".to_string()),
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
        })
        .collect()
}

fn remove_stale_active_port(profile_dir: &Path) -> PlatformResult<()> {
    let path = profile_dir.join(ACTIVE_PORT_FILE_NAME);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(PlatformError::new(
            ErrorCode::Fatal,
            format!("failed to remove stale DevToolsActivePort: {error}"),
        )),
    }
}

fn wait_for_active_port(
    child: &mut Child,
    profile_dir: &Path,
    timeout: Duration,
) -> PlatformResult<ActiveDevToolsPort> {
    let path = profile_dir.join(ACTIVE_PORT_FILE_NAME);
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            PlatformError::new(
                ErrorCode::Fatal,
                format!("failed to inspect browser process: {error}"),
            )
        })? {
            return Err(PlatformError::new(
                ErrorCode::TargetUnresponsive,
                format!("browser exited before CDP readiness with {status}"),
            ));
        }
        if let Some(endpoint) = read_active_port(&path)? {
            return Ok(endpoint);
        }
        if Instant::now() >= deadline {
            return Err(PlatformError::new(
                ErrorCode::TargetUnresponsive,
                "timed out waiting for browser DevToolsActivePort",
            ));
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

fn read_active_port(path: &Path) -> PlatformResult<Option<ActiveDevToolsPort>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) if is_retryable_active_port_open_error(&error) => return Ok(None),
        Err(error) => {
            return Err(PlatformError::new(
                ErrorCode::Fatal,
                format!("failed to open DevToolsActivePort: {error}"),
            ));
        }
    };
    let metadata = file.metadata().map_err(|error| {
        PlatformError::new(
            ErrorCode::Fatal,
            format!("failed to inspect DevToolsActivePort: {error}"),
        )
    })?;
    if metadata.len() > MAX_ACTIVE_PORT_BYTES as u64 {
        return Err(invalid_ready_file("DevToolsActivePort exceeds 4 KiB"));
    }
    let initial_capacity = usize::try_from(metadata.len())
        .map_or(MAX_ACTIVE_PORT_BYTES, |length| {
            length.min(MAX_ACTIVE_PORT_BYTES)
        });
    let mut bytes = Vec::with_capacity(initial_capacity);
    file.by_ref()
        .take((MAX_ACTIVE_PORT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            PlatformError::new(
                ErrorCode::Fatal,
                format!("failed to read DevToolsActivePort: {error}"),
            )
        })?;
    parse_active_port(&bytes)
}

// Windows can expose the file before the browser releases its exclusive write
// or lock. Those two errors are readiness signals, not terminal open failures.
fn is_retryable_active_port_open_error(error: &std::io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(WINDOWS_ERROR_SHARING_VIOLATION | WINDOWS_ERROR_LOCK_VIOLATION)
    )
}

fn parse_active_port(bytes: &[u8]) -> PlatformResult<Option<ActiveDevToolsPort>> {
    if bytes.len() > MAX_ACTIVE_PORT_BYTES {
        return Err(invalid_ready_file("DevToolsActivePort exceeds 4 KiB"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid_ready_file("DevToolsActivePort is not UTF-8"))?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let Some(port_text) = lines.next() else {
        return Ok(None);
    };
    let Some(browser_path) = lines.next() else {
        return Ok(None);
    };
    if lines.next().is_some() {
        return Err(invalid_ready_file(
            "DevToolsActivePort must contain exactly two non-empty lines",
        ));
    }
    let port = port_text
        .trim()
        .parse::<u16>()
        .map_err(|_| invalid_ready_file("DevToolsActivePort port is invalid"))?;
    if port == 0 {
        return Err(invalid_ready_file("DevToolsActivePort port is zero"));
    }
    let browser_path = browser_path.trim();
    if browser_path.len() > MAX_BROWSER_PATH_BYTES
        || !browser_path.starts_with("/devtools/browser/")
        || !browser_path.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '/' | '-' | '_' | '.')
        })
    {
        return Err(invalid_ready_file(
            "DevToolsActivePort browser path is invalid",
        ));
    }
    Ok(Some(ActiveDevToolsPort {
        port,
        browser_path: browser_path.to_string(),
    }))
}

fn terminate_child_tree(child: &mut Child, timeout: Duration) -> PlatformResult<()> {
    if child
        .try_wait()
        .map_err(|error| process_error("inspect process before termination", &error))?
        .is_some()
    {
        return Ok(());
    }

    let taskkill_outcome = Command::new("taskkill.exe")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let wait_outcome = wait_for_exit(child, timeout)?;
    if wait_outcome.is_some() {
        return match taskkill_outcome {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(PlatformError::new(
                ErrorCode::TargetUnresponsive,
                format!("taskkill returned {status}, but the browser root was reaped"),
            )),
            Err(error) => Err(process_error("run taskkill", &error)),
        };
    }

    let kill_outcome = child.kill();
    let final_wait = wait_for_exit(child, Duration::from_secs(2))?;
    if final_wait.is_some() {
        let context = match kill_outcome {
            Ok(()) => "taskkill did not reap the browser; fallback kill succeeded".to_string(),
            Err(error) => format!("taskkill and fallback kill failed: {error}"),
        };
        return Err(PlatformError::new(ErrorCode::TargetUnresponsive, context));
    }
    Err(PlatformError::new(
        ErrorCode::TargetUnresponsive,
        "browser process tree did not terminate within the bounded wait",
    ))
}

fn wait_for_exit(
    child: &mut Child,
    timeout: Duration,
) -> PlatformResult<Option<std::process::ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| process_error("wait for browser process", &error))?
        {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

fn terminate_child_tree_best_effort(child: &mut Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    let taskkill_succeeded = Command::new("taskkill.exe")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !taskkill_succeeded && !matches!(child.try_wait(), Ok(Some(_))) {
        let kill_succeeded = child.kill().is_ok();
        if !kill_succeeded {
            return;
        }
    }
    let deadline = Instant::now() + TERMINATE_TIMEOUT;
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

fn with_cleanup_context(start_error: PlatformError, cleanup: PlatformResult<()>) -> PlatformError {
    match cleanup {
        Ok(()) => start_error,
        Err(cleanup_error) => PlatformError::new(
            start_error.code(),
            format!(
                "{}; cleanup also failed: {}",
                start_error.message(),
                cleanup_error.message()
            ),
        ),
    }
}

fn invalid_args(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::ToolInvalidArgs, message)
}

fn invalid_ready_file(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::ToolInvalidArgs, message)
}

fn process_error(action: &str, error: &std::io::Error) -> PlatformError {
    PlatformError::new(ErrorCode::Fatal, format!("{action}: {error}"))
}

#[cfg(test)]
#[path = "browser_launcher_tests.rs"]
mod tests;

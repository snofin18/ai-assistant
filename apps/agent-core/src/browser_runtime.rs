//! Private CLI assembly for a dedicated Edge/Chrome CDP runtime.
//!
//! Responsibilities:
//! - parse explicit browser executable and dedicated-profile inputs;
//! - start one browser, connect to its dynamic loopback CDP port, and build the
//!   six-tool browser registry;
//! - keep the launched process alive through registry assembly and close it
//!   explicitly after the CLI self-check.
//!
//! Boundaries:
//! - does not start a browser during tests;
//! - does not execute a task, visit an external site, or use user login state;
//! - does not expose a second public production composition-root contract.
//!
//! Invariants:
//! 1. the browser always uses a caller-provided dedicated profile root;
//! 2. CDP discovery is loopback-only and uses the launcher-published port;
//! 3. a registry or connection failure drops `LaunchedBrowser`, which reclaims
//!    the complete browser process tree.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use assistant_agent_core::browser_registry::{
    BrowserFamily, BrowserRegistryError, build_browser_registry,
};
use assistant_browser_profile::{BrowserKind, BrowserProfile};
use assistant_platform_api::{CdpConnectOptions, CdpProvider, ErrorCode, PlatformError};
use assistant_platform_windows::browser_launcher::{BrowserLauncher, LaunchedBrowser};
use assistant_platform_windows::cdp::runtime::RuntimeCdpProvider;
use assistant_protocol::ToolSchema;
use assistant_tool_bus::ToolRegistry;
use thiserror::Error;

const DEFAULT_PROFILE_ID: &str = "automation";
const DEFAULT_SESSION_TIMEOUT_MS: u64 = 5_000;
const MIN_SESSION_TIMEOUT_MS: u64 = 100;
const MAX_SESSION_TIMEOUT_MS: u64 = 300_000;
const BROWSER_PAGE_URL: &str = "about:blank";

/// Browser family accepted by the private CLI assembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserSelection {
    /// Microsoft Edge.
    Edge,
    /// Google Chrome.
    Chrome,
}

impl BrowserSelection {
    /// Stable lowercase name used by CLI output and target markers.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Edge => "edge",
            Self::Chrome => "chrome",
        }
    }

    pub(crate) const fn app_id(self) -> &'static str {
        match self {
            Self::Edge => "com.microsoft.edge",
            Self::Chrome => "com.google.chrome",
        }
    }

    const fn browser_kind(self) -> BrowserKind {
        match self {
            Self::Edge => BrowserKind::Edge,
            Self::Chrome => BrowserKind::Chrome,
        }
    }

    const fn registry_family(self) -> BrowserFamily {
        match self {
            Self::Edge => BrowserFamily::Edge,
            Self::Chrome => BrowserFamily::Chrome,
        }
    }

    const fn planner_tool_prefix(self) -> &'static str {
        match self {
            Self::Edge => "edge.cdp.",
            Self::Chrome => "chrome.cdp.",
        }
    }

    fn default_adapter_root(self) -> PathBuf {
        match self {
            Self::Edge => PathBuf::from("adapters/browser.edge"),
            Self::Chrome => PathBuf::from("adapters/browser.chrome"),
        }
    }
}

/// Validated inputs for one CLI browser runtime launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserRuntimeOptions {
    browser: BrowserSelection,
    executable: PathBuf,
    profile_root: PathBuf,
    profile_id: String,
    adapter_root: PathBuf,
    session_timeout_ms: u64,
}

impl BrowserRuntimeOptions {
    /// Parses the arguments after `--browser-runtime-check`.
    ///
    /// Duplicate options and unknown options fail closed rather than silently
    /// choosing one value.
    pub fn parse(arguments: &[String]) -> Result<Self, BrowserRuntimeError> {
        let mut browser = None;
        let mut executable = None;
        let mut profile_root = None;
        let mut profile_id = None;
        let mut adapter_root = None;
        let mut session_timeout_ms = None;
        let mut iterator = arguments.iter();
        while let Some(argument) = iterator.next() {
            match argument.as_str() {
                "--browser" => {
                    set_once(
                        &mut browser,
                        parse_browser(&next_value(&mut iterator, argument)?)?,
                        argument,
                    )?;
                }
                "--browser-executable" => {
                    set_once(
                        &mut executable,
                        PathBuf::from(next_value(&mut iterator, argument)?),
                        argument,
                    )?;
                }
                "--browser-profile-root" => {
                    set_once(
                        &mut profile_root,
                        PathBuf::from(next_value(&mut iterator, argument)?),
                        argument,
                    )?;
                }
                "--browser-profile-id" => {
                    set_once(
                        &mut profile_id,
                        next_value(&mut iterator, argument)?,
                        argument,
                    )?;
                }
                "--browser-adapter-root" => {
                    set_once(
                        &mut adapter_root,
                        PathBuf::from(next_value(&mut iterator, argument)?),
                        argument,
                    )?;
                }
                "--browser-session-timeout-ms" => {
                    let value = next_value(&mut iterator, argument)?;
                    let value = value.parse::<u64>().map_err(|_| {
                        argument_error(format!(
                            "{argument} must be an integer in \
                             {MIN_SESSION_TIMEOUT_MS}..={MAX_SESSION_TIMEOUT_MS}"
                        ))
                    })?;
                    set_once(&mut session_timeout_ms, value, argument)?;
                }
                _ => return Err(argument_error(format!("unknown argument `{argument}`"))),
            }
        }

        let browser = browser.ok_or_else(|| argument_error("missing --browser edge|chrome"))?;
        let session_timeout_ms = session_timeout_ms.unwrap_or(DEFAULT_SESSION_TIMEOUT_MS);
        if !(MIN_SESSION_TIMEOUT_MS..=MAX_SESSION_TIMEOUT_MS).contains(&session_timeout_ms) {
            return Err(argument_error(format!(
                "--browser-session-timeout-ms must be in \
                 {MIN_SESSION_TIMEOUT_MS}..={MAX_SESSION_TIMEOUT_MS}"
            )));
        }
        let options = Self {
            browser,
            executable: require_path(executable, "--browser-executable")?,
            profile_root: require_path(profile_root, "--browser-profile-root")?,
            profile_id: profile_id.unwrap_or_else(|| DEFAULT_PROFILE_ID.to_owned()),
            adapter_root: adapter_root.unwrap_or_else(|| browser.default_adapter_root()),
            session_timeout_ms,
        };
        options.validate()?;
        Ok(options)
    }

    fn tools_path(&self) -> PathBuf {
        self.adapter_root.join("tools").join("tools.json")
    }

    fn validate(&self) -> Result<(), BrowserRuntimeError> {
        if !self.profile_root.is_absolute() {
            return Err(argument_error(
                "--browser-profile-root must be an absolute path",
            ));
        }
        if !self.executable.is_absolute() {
            return Err(argument_error(
                "--browser-executable must be an absolute path",
            ));
        }
        if self.profile_id.trim().is_empty() {
            return Err(argument_error("--browser-profile-id must not be empty"));
        }
        BrowserProfile::new(
            &self.profile_id,
            self.browser.browser_kind(),
            self.profile_root.join(&self.profile_id),
        )
        .map_err(|error| argument_error(format!("--browser-profile-id is invalid: {error}")))?;
        Ok(())
    }
}

/// One live browser and its registry, with browser ownership retained.
pub struct BrowserRuntime {
    browser_process: LaunchedBrowser,
    registry: ToolRegistry,
    tool_schemas: Vec<ToolSchema>,
    browser: BrowserSelection,
}

impl BrowserRuntime {
    /// Starts, connects, and builds the six-tool browser registry.
    ///
    /// If connection or registry construction fails after launch, dropping
    /// `LaunchedBrowser` performs the TASK-110 bounded process-tree cleanup.
    pub async fn launch(options: BrowserRuntimeOptions) -> Result<Self, BrowserRuntimeError> {
        let launch_configuration = BrowserLauncher::new(
            &options.executable,
            &options.profile_root,
            options.profile_id.clone(),
            options.browser.browser_kind(),
        )?;
        let browser_process = launch_configuration.launch()?;
        let provider = Arc::new(RuntimeCdpProvider::new());
        let connect_options = CdpConnectOptions::new(
            "127.0.0.1",
            browser_process.cdp_port(),
            BROWSER_PAGE_URL,
            None,
            Some(format!(
                "{}:{}",
                options.browser.as_str(),
                options.profile_id
            )),
            options.session_timeout_ms,
        )?;
        let session = provider.connect(&connect_options).await?;
        let build = build_browser_registry(
            Arc::clone(&provider),
            session.target(),
            options.browser.registry_family(),
            &options.tools_path(),
        )
        .map_err(|error| BrowserRuntimeError::from_registry(&error))?;
        let (registry, tool_schemas) = build.into_parts();
        let browser_schema_count = tool_schemas
            .iter()
            .filter(|schema| {
                schema
                    .name
                    .starts_with(options.browser.planner_tool_prefix())
            })
            .count();
        if registry.len() != 6 || browser_schema_count != 6 {
            return Err(BrowserRuntimeError::Assembly {
                reason: format!(
                    "browser registry must contain exactly six tools, got {browser_schema_count} \
                     browser schemas and {} tools",
                    registry.len(),
                ),
            });
        }
        Ok(Self {
            browser_process,
            registry,
            tool_schemas,
            browser: options.browser,
        })
    }

    /// Consumes the self-check wrapper while retaining the browser lifecycle.
    pub(crate) fn into_parts(
        self,
    ) -> (
        LaunchedBrowser,
        ToolRegistry,
        Vec<ToolSchema>,
        BrowserSelection,
    ) {
        (
            self.browser_process,
            self.registry,
            self.tool_schemas,
            self.browser,
        )
    }

    /// Closes the browser process tree and returns the deterministic CLI report.
    pub fn shutdown(mut self) -> Result<BrowserRuntimeReport, BrowserRuntimeError> {
        let report = BrowserRuntimeReport {
            browser: self.browser,
            tool_count: self.registry.len(),
            schema_count: self.tool_schemas.len(),
        };
        self.browser_process.close()?;
        Ok(report)
    }
}

/// Successful browser-runtime check result.
pub struct BrowserRuntimeReport {
    browser: BrowserSelection,
    tool_count: usize,
    schema_count: usize,
}

impl BrowserRuntimeReport {
    /// Stable one-line CLI representation.
    pub fn display_line(&self) -> String {
        format!(
            "assistant-agent-core browser runtime: browser={} \
             browser_tools={} planner_schemas={} status=closed",
            self.browser.as_str(),
            self.tool_count,
            self.schema_count
        )
    }
}

/// Runs the private CLI mode and maps every failure to an explicit exit code.
pub async fn run_cli(arguments: &[String]) -> ExitCode {
    let rest = arguments.get(1..).unwrap_or(&[]);
    match run_check(rest).await {
        Ok(report) => {
            if writeln!(io::stdout().lock(), "{}", report.display_line()).is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "browser runtime [{:?}]: {error}",
                error.error_code()
            );
            ExitCode::from(2)
        }
    }
}

async fn run_check(arguments: &[String]) -> Result<BrowserRuntimeReport, BrowserRuntimeError> {
    BrowserRuntime::launch(BrowserRuntimeOptions::parse(arguments)?)
        .await?
        .shutdown()
}

/// Browser-runtime startup and lifecycle failures.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BrowserRuntimeError {
    /// CLI input is missing or unsafe.
    #[error("browser runtime arguments: {reason}")]
    Arguments {
        /// Failure detail.
        reason: String,
    },
    /// Browser launch, CDP connection, or shutdown failed.
    #[error(transparent)]
    Platform(#[from] PlatformError),
    /// Adapter declaration loading or tool registration failed.
    #[error("browser registry assembly failed: {reason}")]
    Registry {
        /// Failure detail.
        reason: String,
    },
    /// Registry returned an unexpected shape.
    #[error("browser runtime assembly failed: {reason}")]
    Assembly {
        /// Failure detail.
        reason: String,
    },
}

impl BrowserRuntimeError {
    /// Stable protocol category for CLI reporting.
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::Arguments { .. } => ErrorCode::ToolInvalidArgs,
            Self::Platform(error) => error.code(),
            Self::Registry { .. } => ErrorCode::CapabilityMissing,
            Self::Assembly { .. } => ErrorCode::Fatal,
        }
    }

    fn from_registry(error: &BrowserRegistryError) -> Self {
        Self::Registry {
            reason: error.to_string(),
        }
    }
}

fn parse_browser(value: &str) -> Result<BrowserSelection, BrowserRuntimeError> {
    match value {
        "edge" => Ok(BrowserSelection::Edge),
        "chrome" => Ok(BrowserSelection::Chrome),
        _ => Err(argument_error(format!(
            "--browser must be `edge` or `chrome`, got `{value}`"
        ))),
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T, option: &str) -> Result<(), BrowserRuntimeError> {
    if slot.replace(value).is_some() {
        return Err(argument_error(format!(
            "{option} was provided more than once"
        )));
    }
    Ok(())
}

fn require_path(value: Option<PathBuf>, option: &str) -> Result<PathBuf, BrowserRuntimeError> {
    value.ok_or_else(|| argument_error(format!("missing {option} <path>")))
}

fn next_value(
    iterator: &mut std::slice::Iter<'_, String>,
    option: &str,
) -> Result<String, BrowserRuntimeError> {
    iterator
        .next()
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .ok_or_else(|| argument_error(format!("{option} requires a value")))
}

fn argument_error(reason: impl Into<String>) -> BrowserRuntimeError {
    BrowserRuntimeError::Arguments {
        reason: reason.into(),
    }
}

#[cfg(test)]
#[path = "browser_runtime_tests.rs"]
mod tests;

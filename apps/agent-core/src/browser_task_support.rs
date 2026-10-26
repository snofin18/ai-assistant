//! Parsing, structured errors, and output-file boundary for the browser task runner.
//!
//! This module deliberately contains no task execution. It owns only the
//! caller-facing CLI shape and the final create-new/read-back output contract.

use std::io::Write;
use std::path::{Path, PathBuf};

use assistant_agent_core::RuntimeExecutionError;
use assistant_platform_api::ErrorCode;
use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::browser_runtime::{BrowserRuntimeError, BrowserRuntimeOptions};

const OUTPUT_SCHEMA_VERSION: u64 = 1;

/// Validated runner inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BrowserTaskOptions {
    pub(super) runtime: BrowserRuntimeOptions,
    pub(super) task_package_path: PathBuf,
    pub(super) task_inputs_path: Option<PathBuf>,
    pub(super) output_path: PathBuf,
}

impl BrowserTaskOptions {
    pub(super) fn parse(arguments: &[String]) -> Result<Self, BrowserTaskError> {
        let mut task_package_path = None;
        let mut task_inputs_path = None;
        let mut output_path = None;
        let mut runtime_arguments = Vec::new();
        let mut index = 0usize;
        while let Some(argument) = arguments.get(index) {
            match argument.as_str() {
                "--task-package" => {
                    set_once(
                        &mut task_package_path,
                        PathBuf::from(next_value(arguments, &mut index, argument)?),
                        argument,
                    )?;
                }
                "--task-inputs" => {
                    set_once(
                        &mut task_inputs_path,
                        PathBuf::from(next_value(arguments, &mut index, argument)?),
                        argument,
                    )?;
                }
                "--output" => {
                    set_once(
                        &mut output_path,
                        PathBuf::from(next_value(arguments, &mut index, argument)?),
                        argument,
                    )?;
                }
                _ => runtime_arguments.push(argument.clone()),
            }
            index = index.saturating_add(1);
        }
        let output_path = output_path.ok_or_else(|| argument_error("missing --output <path>"))?;
        if !output_path.is_absolute() {
            return Err(argument_error("--output must be an absolute path"));
        }
        if output_path.exists() {
            return Err(argument_error(
                "--output already exists; refusing to overwrite",
            ));
        }
        if !output_path.parent().is_some_and(Path::is_dir) {
            return Err(argument_error(
                "--output parent directory must already exist",
            ));
        }
        Ok(Self {
            runtime: BrowserRuntimeOptions::parse(&runtime_arguments)?,
            task_package_path: task_package_path
                .ok_or_else(|| argument_error("missing --task-package <path>"))?,
            task_inputs_path,
            output_path,
        })
    }

    pub(super) fn read_task_inputs(&self) -> Result<Map<String, Value>, BrowserTaskError> {
        let Some(path) = self.task_inputs_path.as_deref() else {
            return Ok(Map::new());
        };
        let body = std::fs::read_to_string(path).map_err(|error| BrowserTaskError::Input {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        let value: Value =
            serde_json::from_str(&body).map_err(|error| BrowserTaskError::Input {
                path: path.display().to_string(),
                reason: format!("task inputs are not valid JSON: {error}"),
            })?;
        value
            .as_object()
            .cloned()
            .ok_or_else(|| BrowserTaskError::Input {
                path: path.display().to_string(),
                reason: "task inputs must be a JSON object".to_owned(),
            })
    }
}

/// Completed execution result before or after file export.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct BrowserTaskResult {
    pub(super) task_id: String,
    pub(super) outputs: Map<String, Value>,
    pub(super) steps: Vec<Value>,
}

impl BrowserTaskResult {
    fn to_json(&self) -> Result<Vec<u8>, BrowserTaskError> {
        let mut bytes = serde_json::to_vec_pretty(&json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "task_id": self.task_id,
            "status": "completed",
            "outputs": self.outputs,
            "steps": self.steps,
        }))
        .map_err(|error| BrowserTaskError::Output {
            path: "<unbound>".to_owned(),
            reason: format!("output serialization failed: {error}"),
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// Runner failures with stable protocol categories.
#[derive(Debug, Error)]
#[non_exhaustive]
pub(super) enum BrowserTaskError {
    #[error("browser task arguments: {reason}")]
    Arguments { reason: String },
    #[error("browser runtime: {0}")]
    Browser(#[from] BrowserRuntimeError),
    #[error("task package: {0}")]
    TaskPackage(#[from] assistant_agent_core::TaskPackageError),
    #[error("task inputs at `{path}`: {reason}")]
    Input { path: String, reason: String },
    #[error("planner: {reason}")]
    Planner { reason: String },
    #[error("task engine: {0}")]
    TaskEngine(#[from] assistant_task_engine::TaskEngineError),
    #[error("runtime: {0}")]
    Runtime(#[from] RuntimeExecutionError),
    #[error("tool bus: {reason}")]
    ToolBus { reason: String },
    #[error("step `{step_id}` ended as {outcome}")]
    StepNotCommitted { step_id: String, outcome: String },
    #[error("declared output `{output}` was not produced by `{step_id}`")]
    MissingOutput { step_id: String, output: String },
    #[error("output file `{path}`: {reason}")]
    Output { reason: String, path: String },
}

impl BrowserTaskError {
    pub(super) const fn error_code(&self) -> ErrorCode {
        match self {
            Self::Arguments { .. } | Self::Input { .. } => ErrorCode::ToolInvalidArgs,
            Self::Browser(_) | Self::TaskPackage(_) | Self::Planner { .. } => {
                ErrorCode::CapabilityMissing
            }
            Self::StepNotCommitted { .. } | Self::MissingOutput { .. } => ErrorCode::VerifyFailed,
            Self::TaskEngine(_) | Self::Runtime(_) | Self::ToolBus { .. } | Self::Output { .. } => {
                ErrorCode::Fatal
            }
        }
    }
}

impl From<assistant_tool_bus::ToolBusError> for BrowserTaskError {
    fn from(error: assistant_tool_bus::ToolBusError) -> Self {
        Self::ToolBus {
            reason: error.to_string(),
        }
    }
}

pub(super) fn write_output(
    path: &Path,
    result: &BrowserTaskResult,
) -> Result<(), BrowserTaskError> {
    let bytes = result.to_json()?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| BrowserTaskError::Output {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| BrowserTaskError::Output {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
    drop(file);
    let observed = std::fs::read(path).map_err(|error| BrowserTaskError::Output {
        path: path.display().to_string(),
        reason: format!("read-back failed: {error}"),
    })?;
    if observed != bytes {
        return Err(BrowserTaskError::Output {
            path: path.display().to_string(),
            reason: "read-back bytes differ from the committed output".to_owned(),
        });
    }
    Ok(())
}

fn set_once<T>(slot: &mut Option<T>, value: T, option: &str) -> Result<(), BrowserTaskError> {
    if slot.replace(value).is_some() {
        return Err(argument_error(format!(
            "{option} was provided more than once"
        )));
    }
    Ok(())
}

fn next_value(
    arguments: &[String],
    index: &mut usize,
    option: &str,
) -> Result<String, BrowserTaskError> {
    *index = index.saturating_add(1);
    arguments
        .get(*index)
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .ok_or_else(|| argument_error(format!("{option} requires a value")))
}

pub(super) fn argument_error(reason: impl Into<String>) -> BrowserTaskError {
    BrowserTaskError::Arguments {
        reason: reason.into(),
    }
}

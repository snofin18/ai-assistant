//! Parsing, structured errors, and output-file boundary for the browser task runner.
//!
//! This module deliberately contains no task execution. It owns only the
//! caller-facing CLI shape and the final create-new/read-back output contract.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use assistant_agent_core::{RuntimeExecutionError, UiServerConfig};
use assistant_platform_api::ErrorCode;
use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::browser_runtime::{BrowserRuntimeError, BrowserRuntimeOptions};

use super::browser_task_csv::{BrowserCsvError, CsvArtifact, CsvOrigin};

const OUTPUT_SCHEMA_VERSION: u64 = 1;

/// Explicit CSV destination and link-resolution origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BrowserTaskCsvOptions {
    pub(super) output_path: PathBuf,
    pub(super) origin: CsvOrigin,
}

/// Validated runner inputs.
#[derive(Debug, Clone)]
pub(super) struct BrowserTaskOptions {
    pub(super) runtime: BrowserRuntimeOptions,
    pub(super) task_package_path: PathBuf,
    pub(super) task_inputs_path: Option<PathBuf>,
    pub(super) output_path: PathBuf,
    pub(super) csv: Option<BrowserTaskCsvOptions>,
    pub(super) approved_step_ids: Vec<String>,
    pub(super) human_approval: bool,
    pub(super) ui_approval: Option<UiServerConfig>,
}

impl BrowserTaskOptions {
    pub(super) fn parse(arguments: &[String]) -> Result<Self, BrowserTaskError> {
        let parsed = parse_arguments(arguments)?;
        validate_unique_step_approvals(&parsed.approved_step_ids)?;
        let ui_approval = parse_ui_approval(&parsed)?;
        let output_path = parsed
            .output_path
            .ok_or_else(|| argument_error("missing --output <path>"))?;
        validate_new_output_path(&output_path, "--output")?;
        let csv = parse_csv_options(parsed.csv_output_path, parsed.csv_base_url, &output_path)?;
        if parsed.human_approval && ui_approval.is_some() {
            return Err(argument_error(
                "--human-approval and --ui-approval are mutually exclusive",
            ));
        }
        Ok(Self {
            runtime: BrowserRuntimeOptions::parse(&parsed.runtime)?,
            task_package_path: parsed
                .task_package_path
                .ok_or_else(|| argument_error("missing --task-package <path>"))?,
            task_inputs_path: parsed.task_inputs_path,
            output_path,
            csv,
            approved_step_ids: parsed.approved_step_ids,
            human_approval: parsed.human_approval,
            ui_approval,
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

#[derive(Default)]
struct ParsedArguments {
    task_package_path: Option<PathBuf>,
    task_inputs_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    runtime: Vec<String>,
    csv_output_path: Option<PathBuf>,
    csv_base_url: Option<String>,
    approved_step_ids: Vec<String>,
    human_approval: bool,
    ui_approval: bool,
    ui_pipe: Option<String>,
    ui_token_environment_variable: Option<String>,
    ui_peer: Option<PathBuf>,
}

impl ParsedArguments {
    fn consume_option(
        &mut self,
        arguments: &[String],
        index: &mut usize,
        option: &str,
    ) -> Result<bool, BrowserTaskError> {
        if self.consume_path_option(arguments, index, option)? {
            return Ok(true);
        }
        if option == "--approve-step" {
            let step_id = next_value(arguments, index, option)?;
            if step_id.trim().is_empty() {
                return Err(argument_error(
                    "--approve-step requires a non-empty step id",
                ));
            }
            self.approved_step_ids.push(step_id);
            return Ok(true);
        }
        if option == "--human-approval" {
            if self.human_approval {
                return Err(argument_error(
                    "--human-approval was provided more than once",
                ));
            }
            self.human_approval = true;
            return Ok(true);
        }
        if option == "--ui-approval" {
            if self.ui_approval {
                return Err(argument_error("--ui-approval was provided more than once"));
            }
            self.ui_approval = true;
            return Ok(true);
        }
        if option == "--ui-pipe" {
            set_once(
                &mut self.ui_pipe,
                next_value(arguments, index, option)?,
                option,
            )?;
            return Ok(true);
        }
        if option == "--ui-token-env" {
            set_once(
                &mut self.ui_token_environment_variable,
                next_value(arguments, index, option)?,
                option,
            )?;
            return Ok(true);
        }
        if option != "--csv-base-url" {
            return Ok(false);
        }
        set_once(
            &mut self.csv_base_url,
            next_value(arguments, index, option)?,
            option,
        )?;
        Ok(true)
    }

    fn consume_path_option(
        &mut self,
        arguments: &[String],
        index: &mut usize,
        option: &str,
    ) -> Result<bool, BrowserTaskError> {
        let slot = match option {
            "--task-package" => &mut self.task_package_path,
            "--task-inputs" => &mut self.task_inputs_path,
            "--output" => &mut self.output_path,
            "--csv-output" => &mut self.csv_output_path,
            "--ui-peer" => &mut self.ui_peer,
            _ => return Ok(false),
        };
        set_once(
            slot,
            PathBuf::from(next_value(arguments, index, option)?),
            option,
        )?;
        Ok(true)
    }
}

fn parse_ui_approval(parsed: &ParsedArguments) -> Result<Option<UiServerConfig>, BrowserTaskError> {
    let has_ui_options = parsed.ui_pipe.is_some()
        || parsed.ui_token_environment_variable.is_some()
        || parsed.ui_peer.is_some();
    if !parsed.ui_approval {
        if has_ui_options {
            return Err(argument_error(
                "--ui-pipe, --ui-token-env, and --ui-peer require --ui-approval",
            ));
        }
        return Ok(None);
    }
    let pipe_name = parsed
        .ui_pipe
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| argument_error("--ui-approval requires --ui-pipe <name>"))?;
    let token_environment_variable = parsed
        .ui_token_environment_variable
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| argument_error("--ui-approval requires --ui-token-env <name>"))?;
    let peer = parsed
        .ui_peer
        .as_ref()
        .ok_or_else(|| argument_error("--ui-approval requires --ui-peer <path>"))?;
    Ok(Some(
        UiServerConfig::new(
            pipe_name,
            token_environment_variable,
            Duration::from_secs(10),
        )
        .with_allowed_peer(peer),
    ))
}

fn validate_unique_step_approvals(step_ids: &[String]) -> Result<(), BrowserTaskError> {
    let mut unique = BTreeSet::new();
    for step_id in step_ids {
        if !unique.insert(step_id.as_str()) {
            return Err(argument_error(format!(
                "--approve-step `{step_id}` was provided more than once"
            )));
        }
    }
    Ok(())
}

fn parse_arguments(arguments: &[String]) -> Result<ParsedArguments, BrowserTaskError> {
    let mut parsed = ParsedArguments::default();
    let mut index = 0usize;
    while let Some(argument) = arguments.get(index) {
        if !parsed.consume_option(arguments, &mut index, argument)? {
            parsed.runtime.push(argument.clone());
        }
        index = index.saturating_add(1);
    }
    Ok(parsed)
}

fn parse_csv_options(
    csv_output_path: Option<PathBuf>,
    csv_base_url: Option<String>,
    output_path: &Path,
) -> Result<Option<BrowserTaskCsvOptions>, BrowserTaskError> {
    match (csv_output_path, csv_base_url) {
        (None, None) => Ok(None),
        (Some(path), Some(origin)) => {
            validate_new_output_path(&path, "--csv-output")?;
            if path == output_path {
                return Err(argument_error("--csv-output must differ from --output"));
            }
            Ok(Some(BrowserTaskCsvOptions {
                output_path: path,
                origin: CsvOrigin::parse(&origin).map_err(|error| {
                    argument_error(format!("--csv-base-url is invalid: {error}"))
                })?,
            }))
        }
        (Some(_), None) => Err(argument_error(
            "--csv-output requires --csv-base-url <origin>",
        )),
        (None, Some(_)) => Err(argument_error(
            "--csv-base-url requires --csv-output <path>",
        )),
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
    fn to_json(
        &self,
        csv_path: Option<&Path>,
        csv: Option<&CsvArtifact>,
    ) -> Result<Vec<u8>, BrowserTaskError> {
        let mut payload = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "task_id": self.task_id,
            "status": "completed",
            "outputs": self.outputs,
            "steps": self.steps,
        });
        if let (Some(path), Some(csv)) = (csv_path, csv) {
            let object = payload
                .as_object_mut()
                .ok_or_else(|| BrowserTaskError::Output {
                    path: path.display().to_string(),
                    reason: "result JSON root is not an object".to_owned(),
                })?;
            object.insert(
                "csv_output".to_owned(),
                json!({
                    "path": path.display().to_string(),
                    "rows": csv.rows,
                    "expected_origin": csv.expected_origin,
                    "encoding": "utf-8",
                    "bom": false,
                    "line_ending": "crlf",
                    "first_product_url": csv.first_product_url,
                    "last_product_url": csv.last_product_url,
                }),
            );
        }
        let mut bytes =
            serde_json::to_vec_pretty(&payload).map_err(|error| BrowserTaskError::Output {
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
    #[error("human rejected form approval for step `{step_id}`")]
    HumanRejected { step_id: String },
    #[error("human approval was not applied for step `{step_id}`")]
    HumanApprovalNotApplied { step_id: String },
    #[error("human approval expired before step `{step_id}` could resume")]
    HumanApprovalExpired { step_id: String },
    #[error("human approval prompt failed: {reason}")]
    HumanPrompt { reason: String },
    #[error("browser UI approval session failed: {reason}")]
    UiServer { reason: String },
    #[error("declared output `{output}` was not produced by `{step_id}`")]
    MissingOutput { step_id: String, output: String },
    #[error("CSV normalization: {0}")]
    Csv(#[from] BrowserCsvError),
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
            Self::HumanRejected { .. } | Self::HumanApprovalExpired { .. } => {
                ErrorCode::UserInteraction
            }
            Self::StepNotCommitted { .. } | Self::MissingOutput { .. } | Self::Csv(_) => {
                ErrorCode::VerifyFailed
            }
            Self::TaskEngine(_)
            | Self::Runtime(_)
            | Self::ToolBus { .. }
            | Self::Output { .. }
            | Self::HumanApprovalNotApplied { .. }
            | Self::HumanPrompt { .. }
            | Self::UiServer { .. } => ErrorCode::Fatal,
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
    json_path: &Path,
    result: &BrowserTaskResult,
    csv_path: Option<&Path>,
    csv: Option<&CsvArtifact>,
) -> Result<(), BrowserTaskError> {
    let bytes = result.to_json(csv_path, csv)?;
    write_new_file(json_path, &bytes)?;
    let Some((csv_path, csv)) = csv_path.zip(csv) else {
        return Ok(());
    };
    if let Err(error) = write_new_file(csv_path, &csv.bytes) {
        return Err(cleanup_after_failure(&[json_path], error));
    }
    Ok(())
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), BrowserTaskError> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| BrowserTaskError::Output {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
    let write_result = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| BrowserTaskError::Output {
            path: path.display().to_string(),
            reason: error.to_string(),
        });
    if let Err(error) = write_result {
        return Err(cleanup_after_failure(&[path], error));
    }
    drop(file);
    let observed = match std::fs::read(path) {
        Ok(observed) => observed,
        Err(error) => {
            return Err(cleanup_after_failure(
                &[path],
                BrowserTaskError::Output {
                    path: path.display().to_string(),
                    reason: format!("read-back failed: {error}"),
                },
            ));
        }
    };
    if observed != bytes {
        return Err(cleanup_after_failure(
            &[path],
            BrowserTaskError::Output {
                path: path.display().to_string(),
                reason: "read-back bytes differ from the committed output".to_owned(),
            },
        ));
    }
    Ok(())
}

fn cleanup_after_failure(paths: &[&Path], error: BrowserTaskError) -> BrowserTaskError {
    let cleanup_errors = paths
        .iter()
        .filter_map(|path| {
            std::fs::remove_file(path)
                .err()
                .filter(|error| error.kind() != std::io::ErrorKind::NotFound)
                .map(|error| format!("{}: {error}", path.display()))
        })
        .collect::<Vec<_>>();
    if cleanup_errors.is_empty() {
        error
    } else {
        BrowserTaskError::Output {
            path: "<cleanup>".to_owned(),
            reason: format!(
                "{error}; additionally failed to remove created files: {}",
                cleanup_errors.join(", ")
            ),
        }
    }
}

fn validate_new_output_path(path: &Path, option: &str) -> Result<(), BrowserTaskError> {
    if !path.is_absolute() {
        return Err(argument_error(format!("{option} must be an absolute path")));
    }
    if path.exists() {
        return Err(argument_error(format!(
            "{option} already exists; refusing to overwrite"
        )));
    }
    if !path.parent().is_some_and(Path::is_dir) {
        return Err(argument_error(format!(
            "{option} parent directory must already exist"
        )));
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

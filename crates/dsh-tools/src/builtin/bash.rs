use crate::error::{Result, ToolError};
use crate::spill::{process_output_spill, SpillConfig};
use crate::tool::Tool;
use async_trait::async_trait;
use dsh_types::{ToolCallId, ToolResultMessage};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

/// Built-in tool for running shell commands.
pub struct BashTool {
    default_timeout: Duration,
    spill_config: SpillConfig,
    default_workdir: Option<PathBuf>,
}

impl Default for BashTool {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(60),
            spill_config: SpillConfig::default(),
            default_workdir: None,
        }
    }
}

impl BashTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn with_spill_config(mut self, config: SpillConfig) -> Self {
        self.spill_config = config;
        self
    }

    pub fn with_workdir(mut self, workdir: impl Into<PathBuf>) -> Self {
        self.default_workdir = Some(workdir.into());
        self
    }
}

#[derive(Debug, Deserialize)]
struct BashArgs {
    cmd: String,
    timeout_secs: Option<u64>,
    workdir: Option<String>,
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command in a bash subshell with timeout, exit code tracking, and output spill protection."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "cmd": {
                    "type": "string",
                    "description": "Shell command to execute."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Optional timeout in seconds. Defaults to 60."
                },
                "workdir": {
                    "type": "string",
                    "description": "Optional working directory."
                }
            },
            "required": ["cmd"]
        })
    }

    async fn execute(&self, call_id: &ToolCallId, args: serde_json::Value) -> Result<ToolResultMessage> {
        let parsed: BashArgs = serde_json::from_value(args).map_err(|e| ToolError::InvalidParameters {
            name: self.name().to_string(),
            reason: e.to_string(),
        })?;

        let timeout_duration = parsed
            .timeout_secs
            .map(Duration::from_secs)
            .unwrap_or(self.default_timeout);

        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&parsed.cmd);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(wd) = parsed.workdir {
            cmd.current_dir(wd);
        } else if let Some(ref wd) = self.default_workdir {
            cmd.current_dir(wd);
        }

        let child = cmd.spawn().map_err(ToolError::Io)?;

        let execution = async move {
            let output = child.wait_with_output().await?;
            Ok::<_, std::io::Error>(output)
        };

        match tokio::time::timeout(timeout_duration, execution).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                let mut combined = String::new();
                if !stdout.is_empty() {
                    combined.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !combined.is_empty() && !combined.ends_with('\n') {
                        combined.push('\n');
                    }
                    combined.push_str(&stderr);
                }

                let processed_output = process_output_spill(&combined, &self.spill_config)
                    .map_err(ToolError::Io)?;

                let exit_code = output.status.code().unwrap_or(-1);
                if output.status.success() {
                    Ok(ToolResultMessage::success(
                        call_id.clone(),
                        if processed_output.is_empty() {
                            "(Command completed with no output)".to_string()
                        } else {
                            processed_output
                        },
                    ))
                } else {
                    let formatted = format!(
                        "Process exited with status code {}:\n{}",
                        exit_code, processed_output
                    );
                    Ok(ToolResultMessage::error(call_id.clone(), formatted))
                }
            }
            Ok(Err(e)) => Err(ToolError::Io(e)),
            Err(_) => Err(ToolError::Timeout {
                timeout_secs: timeout_duration.as_secs(),
            }),
        }
    }
}


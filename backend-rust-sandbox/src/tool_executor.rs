use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::process::Command;

use crate::tool_web_search;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolLanguage {
	Rust,
	Go,
	Python,
	Java,
}

#[derive(Debug, Serialize)]
pub struct ToolExecutionResult {
	pub status: String,
	pub language: ToolLanguage,
	pub stdout: String,
	pub stderr: String,
	pub compile_stdout: String,
	pub compile_stderr: String,
	pub exit_code: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ToolResult {
	pub status: String,
	pub stdout: String,
	pub stderr: String,
}

#[derive(Debug)]
struct CmdOutput {
	stdout: String,
	stderr: String,
	exit_code: Option<i32>,
	success: bool,
}

async fn run_cmd(program: &str, args: &[&str], cwd: &Path) -> std::io::Result<CmdOutput> {
	let out = Command::new(program).args(args).current_dir(cwd).output().await?;
	Ok(CmdOutput {
		stdout: String::from_utf8_lossy(&out.stdout).to_string(),
		stderr: String::from_utf8_lossy(&out.stderr).to_string(),
		exit_code: out.status.code(),
		success: out.status.success(),
	})
}

fn make_run_dir() -> PathBuf {
	let nanos = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_nanos();
	PathBuf::from("sandbox_runs").join(format!("run-{nanos}"))
}

/// Execute a tool request.
///
/// Dispatch order:
/// 1) Internal tools (e.g. web_search)
/// 2) Source-code execution tools (e.g. execute_code)
pub async fn execute_tool(name: &str, args: Value) -> ToolResult {
	let internal = execute_internal_tool(name, args.clone()).await;
	if internal.status != "not_internal" {
		return internal;
	}

	match name {
		"execute_code" => {
			let lang_value = args.get("language").cloned().unwrap_or(json!("python"));
			let language: ToolLanguage =
				serde_json::from_value(lang_value).unwrap_or(ToolLanguage::Python);

			let code = args.get("code").and_then(|v| v.as_str()).unwrap_or("");
			let exec = execute_code(language, code).await;

			let stdout = serde_json::to_string_pretty(&exec).unwrap_or_else(|_| exec.stdout.clone());
			let stderr = format!(
				"{}{}{}",
				exec.compile_stderr,
				if exec.compile_stderr.is_empty() { "" } else { "\n" },
				exec.stderr
			);

			ToolResult {
				status: exec.status,
				stdout,
				stderr,
			}
		}
		"weather_tool" => {
			let city = args.get("city").and_then(|v| v.as_str()).unwrap_or("unknown");
			ToolResult {
				status: "ok".to_string(),
				stdout: serde_json::to_string_pretty(&json!({
					"city": city,
					"temperature": "22C",
					"conditions": "Sunny",
				}))
				.unwrap_or_else(|_| format!("{{\"city\":\"{}\"}}", city)),
				stderr: "".to_string(),
			}
		}
		_ => ToolResult {
			status: "unknown_tool".to_string(),
			stdout: serde_json::to_string_pretty(&json!({
				"message": "Unknown tool",
				"tool_name": name,
				"echo": args,
			}))
			.unwrap_or_else(|_| "Unknown tool".to_string()),
			stderr: "".to_string(),
		},
	}
}

pub async fn execute_internal_tool(name: &str, args: Value) -> ToolResult {
	if name == "web_search" {
		return tool_web_search::execute_web_search(args).await;
	}

	ToolResult {
		status: "not_internal".to_string(),
		stdout: "".to_string(),
		stderr: "".to_string(),
	}
}

/// Execute source code in the requested language.
async fn execute_code(language: ToolLanguage, source_code: &str) -> ToolExecutionResult {
	match language {
		ToolLanguage::Java => execute_java_tool(source_code).await,
		_ => ToolExecutionResult {
			status: "unsupported_language".to_string(),
			language,
			stdout: "".to_string(),
			stderr: "".to_string(),
			compile_stdout: "".to_string(),
			compile_stderr: format!("Language {language:?} not implemented in sandbox yet"),
			exit_code: None,
		},
	}
}

async fn execute_java_tool(source_code: &str) -> ToolExecutionResult {
	let run_dir = make_run_dir();
	if let Err(e) = fs::create_dir_all(&run_dir).await {
		return ToolExecutionResult {
			status: "io_error".to_string(),
			language: ToolLanguage::Java,
			stdout: "".to_string(),
			stderr: "".to_string(),
			compile_stdout: "".to_string(),
			compile_stderr: format!("failed to create run dir: {e}"),
			exit_code: None,
		};
	}

	let java_path = run_dir.join("Tool.java");
	if let Err(e) = fs::write(&java_path, source_code).await {
		return ToolExecutionResult {
			status: "io_error".to_string(),
			language: ToolLanguage::Java,
			stdout: "".to_string(),
			stderr: "".to_string(),
			compile_stdout: "".to_string(),
			compile_stderr: format!("failed to write Tool.java: {e}"),
			exit_code: None,
		};
	}

	let compile = match run_cmd("javac", &["Tool.java"], &run_dir).await {
		Ok(o) => o,
		Err(e) => {
			return ToolExecutionResult {
				status: "compile_error".to_string(),
				language: ToolLanguage::Java,
				stdout: "".to_string(),
				stderr: "".to_string(),
				compile_stdout: "".to_string(),
				compile_stderr: format!("failed to run javac: {e}"),
				exit_code: None,
			}
		}
	};

	if !compile.success {
		return ToolExecutionResult {
			status: "compile_error".to_string(),
			language: ToolLanguage::Java,
			stdout: "".to_string(),
			stderr: "".to_string(),
			compile_stdout: compile.stdout,
			compile_stderr: compile.stderr,
			exit_code: compile.exit_code,
		};
	}

	let run = match run_cmd("java", &["Tool"], &run_dir).await {
		Ok(o) => o,
		Err(e) => {
			return ToolExecutionResult {
				status: "runtime_error".to_string(),
				language: ToolLanguage::Java,
				stdout: "".to_string(),
				stderr: format!("failed to run java: {e}"),
				compile_stdout: compile.stdout,
				compile_stderr: compile.stderr,
				exit_code: None,
			};
		}
	};

	ToolExecutionResult {
		status: if run.success {
			"ok".to_string()
		} else {
			"runtime_error".to_string()
		},
		language: ToolLanguage::Java,
		stdout: run.stdout,
		stderr: run.stderr,
		compile_stdout: compile.stdout,
		compile_stderr: compile.stderr,
		exit_code: run.exit_code,
	}
}


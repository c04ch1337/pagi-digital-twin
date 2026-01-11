use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::info;

use crate::tool_executor::execute_tool;

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolExecutionRequest {
    pub tool_name: String,
    pub args: Value,
}

#[derive(Debug, Serialize)]
pub struct ToolExecutionResponse {
    pub status: String,
    pub tool_name: String,
    pub result: Value,
}

pub async fn execute_mock_tool(req: ToolExecutionRequest) -> ToolExecutionResponse {
    info!(
        tool_name = req.tool_name,
        args = %req.args,
        message = "Executing mock tool"
    );

	let tool_result = execute_tool(req.tool_name.as_str(), req.args.clone()).await;
	let parsed_stdout: Value = serde_json::from_str(&tool_result.stdout)
		.unwrap_or_else(|_| json!({"stdout": tool_result.stdout}));
	let result = json!({
		"stdout": parsed_stdout,
		"stderr": tool_result.stderr,
	});

	ToolExecutionResponse {
		status: tool_result.status,
		tool_name: req.tool_name,
		result,
	}
}


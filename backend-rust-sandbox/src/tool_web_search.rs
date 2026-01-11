use crate::tool_executor::ToolResult;
use serde_json::Value;

/// Implements the `web_search` tool.
///
/// This is a deterministic "mock" backed by a public JSON endpoint so we can
/// exercise real HTTP I/O without requiring a search API key.
///
/// Args (optional):
///   {"query": "..."}
pub async fn execute_web_search(args: Value) -> ToolResult {
	let _query = args
		.get("query")
		.and_then(|v| v.as_str())
		.unwrap_or_default();

	let url = "https://jsonplaceholder.typicode.com/posts/1";
	let client = reqwest::Client::new();

	let resp = match client.get(url).send().await {
		Ok(r) => r,
		Err(e) => {
			return ToolResult {
				status: "network_error".to_string(),
				stdout: "".to_string(),
				stderr: format!("reqwest error: {e}"),
			};
		}
	};

	let status = resp.status();
	let body_text = match resp.text().await {
		Ok(t) => t,
		Err(e) => {
			return ToolResult {
				status: "read_error".to_string(),
				stdout: "".to_string(),
				stderr: format!("failed reading response body: {e}"),
			};
		}
	};

	let stdout = match serde_json::from_str::<Value>(&body_text) {
		Ok(v) => serde_json::to_string_pretty(&v).unwrap_or(body_text),
		Err(_) => body_text,
	};

	ToolResult {
		status: if status.is_success() {
			"ok".to_string()
		} else {
			format!("http_{}", status.as_u16())
		},
		stdout,
		stderr: "".to_string(),
	}
}


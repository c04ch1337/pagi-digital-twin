use serde_json::{json, Value};
use tonic::{Request, Response, Status};
use tracing::info;

use crate::tool_executor;

pub mod proto {
	tonic::include_proto!("modelgateway");
}

use proto::tool_service_server::{ToolService, ToolServiceServer};
use proto::{ToolRequest, ToolResponse};

#[derive(Debug, Default)]
pub struct SandboxToolService;

#[tonic::async_trait]
impl ToolService for SandboxToolService {
	async fn execute_tool(
		&self,
		request: Request<ToolRequest>,
	) -> Result<Response<ToolResponse>, Status> {
		let req = request.into_inner();

		// Log the (future) isolation/resource contract fields. The current sandbox
		// implementation may ignore these, but they are required for a hardened
		// micro-VM execution architecture (gVisor/Firecracker).
		info!(
			tool_name = %req.tool_name,
			execution_environment = %req.execution_environment,
			cpu_limit_mhz = req.cpu_limit_mhz,
			memory_limit_mb = req.memory_limit_mb,
			timeout_seconds = req.timeout_seconds,
			message = "Received ToolRequest (isolation contract)"
		);

		let args: Value = if req.args_json.trim().is_empty() {
			json!({})
		} else {
			serde_json::from_str(&req.args_json)
				.map_err(|e| Status::invalid_argument(format!("invalid args_json: {e}")))?
		};

		let result = tool_executor::execute_tool(req.tool_name.as_str(), args).await;

		Ok(Response::new(ToolResponse {
			status: result.status,
			stdout: result.stdout,
			stderr: result.stderr,
		}))
	}
}

pub fn tool_service_server() -> ToolServiceServer<SandboxToolService> {
	ToolServiceServer::new(SandboxToolService::default())
}


from fastapi import FastAPI, Request
import uvicorn
import os
import time
import json
import httpx
from datetime import datetime
from pydantic import BaseModel

from agent_loop import AgentLoop
from tool_executor import execute_tool
from memory_client import store_session_history
from instrumentation import setup_tracing

from opentelemetry.instrumentation.fastapi import FastAPIInstrumentor

app = FastAPI(title="Python Agent Orchestrator")
SERVICE_NAME = "backend-python-agent"
VERSION = "1.0.0"

tracer = setup_tracing(SERVICE_NAME)
FastAPIInstrumentor.instrument_app(app)

# Environment variables
GO_BFF_URL = os.environ.get("GO_BFF_URL", "http://localhost:8002")
REQUEST_TIMEOUT = float(os.environ.get("REQUEST_TIMEOUT_SECONDS", 2))
PORT = int(os.environ.get("PY_AGENT_PORT", 8000))


class PlanRequest(BaseModel):
    prompt: str = "Generate a 3-step plan to solve X."


def execute_tool_handler(tool_name: str, args: dict) -> dict:
    """Thin wrapper around the Rust Sandbox tool execution call.

    This is the function the future agentic loop will call.
    """

    return execute_tool(tool_name, args)


agent_loop = AgentLoop(tool_executor=execute_tool_handler, max_turns=3)


# Middleware for structured JSON logging
@app.middleware("http")
async def log_requests(request: Request, call_next):
    start_time = time.time()
    response = await call_next(request)
    process_time = time.time() - start_time

    log_entry = {
        "timestamp": datetime.now().isoformat(),
        "level": "info",
        "service": SERVICE_NAME,
        "method": request.method,
        "path": request.url.path,
        "status": response.status_code,
        "latency_ms": round(process_time * 1000, 2),
        "request_id": request.headers.get("X-Request-Id", "none"),
    }
    print(json.dumps(log_entry))
    return response


@app.get("/health")
def health_check():
    return {"service": SERVICE_NAME, "status": "ok", "version": VERSION}


@app.post("/api/v1/plan")
async def create_agent_plan(request: Request, plan_request: PlanRequest):
    """Simulates agent planning. Calls Go BFF /echo to confirm wiring."""

    with tracer.start_as_current_span("AgentPlanExecution") as span:
        request_id = request.headers.get(
            "X-Request-Id", "generated-python-" + str(int(time.time()))
        )
        span.set_attribute("http.request_id", request_id)
        span.set_attribute("agent.prompt", plan_request.prompt)

        bff_echo_data = {}

        # 1. Call Go BFF /echo to confirm reverse wiring (non-recursive check)
        try:
            async with httpx.AsyncClient(timeout=REQUEST_TIMEOUT) as client:
                headers = {"X-Request-Id": request_id}
                response = await client.post(
                    f"{GO_BFF_URL}/api/v1/echo",
                    json={"ping": SERVICE_NAME, "request_id": request_id},
                    headers=headers,
                )
                response.raise_for_status()
                bff_echo_data = response.json()
        except httpx.RequestError as e:
            bff_echo_data = {
                "error": f"BFF connection error: {e.__class__.__name__}",
                "url": f"{GO_BFF_URL}/api/v1/echo",
            }
        except httpx.HTTPStatusError as e:
            bff_echo_data = {
                "error": f"BFF HTTP error: {e.response.status_code}",
                "url": f"{GO_BFF_URL}/api/v1/echo",
            }

        # 2. Run the agent loop (currently 1 turn of LLM call; loop structure ready)
        agent_result = await agent_loop.run_agent(
            plan_request.prompt,
            context={"request_id": request_id, "session_id": request_id},
        )

        # 2b. Persist the full transaction back to Memory (best-effort).
        await store_session_history(
            session_id=request_id,
            history=agent_result.get("history", []),
            prompt=plan_request.prompt,
            llm_response=agent_result.get("llm_response", {}),
        )

        response_payload = {
            "service": SERVICE_NAME,
            "status": "ok",
            **agent_result,
            "bff_echo": bff_echo_data,
            "request_id": request_id,
        }

        # 3. Return plan payload
        return response_payload


if __name__ == "__main__":
    uvicorn.run("main:app", host="0.0.0.0", port=PORT, reload=False)


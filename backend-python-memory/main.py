from __future__ import annotations

import json
import os
from datetime import datetime
from typing import Any

import uvicorn
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

from opentelemetry import trace
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.instrumentation.fastapi import FastAPIInstrumentor
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor

from memory_service import (
    check_health,
    get_mock_session_history,
    start_grpc_server_background,
    store_mind_playbook,
)


app = FastAPI(title="Python Memory Service")
SERVICE_NAME = "backend-python-memory"
VERSION = "1.0.0"


def setup_tracing(service_name: str):
    endpoint = os.environ.get("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4317")
    provider = TracerProvider(resource=Resource.create({"service.name": service_name}))
    trace.set_tracer_provider(provider)
    provider.add_span_processor(
        BatchSpanProcessor(OTLPSpanExporter(endpoint=endpoint, insecure=True))
    )


setup_tracing(SERVICE_NAME)
FastAPIInstrumentor.instrument_app(app)


@app.on_event("startup")
async def _startup():
    grpc_port = int(os.environ.get("MEMORY_GRPC_PORT", "50052"))
    start_grpc_server_background(port=grpc_port)

PORT = int(os.environ.get("MEMORY_PORT", 8003))


class StoreHistoryPayload(BaseModel):
    session_id: str
    history: list[dict[str, Any]]
    prompt: str
    llm_response: dict[str, Any]
    stored_at: str | None = None


class StorePlaybookPayload(BaseModel):
    session_id: str
    prompt: str
    history_sequence: list[dict[str, str]]


@app.get("/health")
def health_check():
    ok, msg = check_health()
    if not ok:
        raise HTTPException(status_code=503, detail=msg)
    return {"service": SERVICE_NAME, "status": "ok", "version": VERSION}


@app.get("/memory/latest")
def get_latest_memory(session_id: str):
    messages = get_mock_session_history(session_id)
    return {"session_id": session_id, "messages": messages}


@app.post("/memory/store")
def store_memory(payload: StoreHistoryPayload):
    """Accept and "store" session history.

    Persistence is simulated for now by printing a structured summary.
    """

    turns = len(payload.history) if isinstance(payload.history, list) else 0
    log_entry = {
        "timestamp": datetime.utcnow().isoformat() + "Z",
        "level": "info",
        "service": SERVICE_NAME,
        "method": "POST /memory/store",
        "session_id": payload.session_id,
        "turns": turns,
        "message": "received session history for persistence (simulated)",
    }
    print(json.dumps(log_entry))


    return {"status": "ok", "session_id": payload.session_id, "turns": turns}


@app.post("/memory/playbook")
def store_playbook(payload: StorePlaybookPayload):
    """Persist a successful multi-step tool sequence into Mind-KB.

    This is called by the Go Agent Planner when it detects successful completion
    after one or more tool calls.
    """

    playbook_id = store_mind_playbook(
        session_id=payload.session_id,
        prompt=payload.prompt,
        history_sequence=payload.history_sequence,
    )
    return {"status": "ok", "playbook_id": playbook_id}


if __name__ == "__main__":
    uvicorn.run("main:app", host="0.0.0.0", port=PORT, reload=False)


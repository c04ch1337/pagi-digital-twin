from __future__ import annotations

import json
import os
from datetime import datetime
from typing import Any

import httpx


async def get_session_history(session_id: str) -> list[dict[str, Any]]:
    """Fetch session history from the Memory service.

    Calls: GET {MEMORY_URL}/memory/latest?session_id={session_id}
    Returns the `messages` list from the JSON response, or [] on failure.
    """

    memory_url = os.environ.get("MEMORY_URL", "http://localhost:8003")
    timeout_seconds = float(os.environ.get("REQUEST_TIMEOUT_SECONDS", 2))

    try:
        async with httpx.AsyncClient(timeout=timeout_seconds) as client:
            resp = await client.get(
                f"{memory_url.rstrip('/')}/memory/latest",
                params={"session_id": session_id},
            )
            resp.raise_for_status()
            data = resp.json()
            messages = data.get("messages", []) if isinstance(data, dict) else []
            return messages if isinstance(messages, list) else []
    except Exception:
        return []


async def store_session_history(
    session_id: str,
    history: list[dict[str, Any]],
    prompt: str,
    llm_response: dict[str, Any],
) -> bool:
    """Persist session history to the Memory service.

    Calls: POST {MEMORY_URL}/memory/store

    This is best-effort: returns False on any failure, True on success.
    """

    memory_url = os.environ.get("MEMORY_URL", "http://localhost:8003")
    timeout_seconds = float(os.environ.get("REQUEST_TIMEOUT_SECONDS", 2))

    payload: dict[str, Any] = {
        "session_id": session_id,
        "prompt": prompt,
        "history": history,
        "llm_response": llm_response,
        "stored_at": datetime.utcnow().isoformat() + "Z",
    }

    try:
        async with httpx.AsyncClient(timeout=timeout_seconds) as client:
            resp = await client.post(
                f"{memory_url.rstrip('/')}/memory/store",
                json=payload,
            )
            resp.raise_for_status()
            return True
    except Exception as e:
        # Log but do not raise; persistence is non-critical for the current loop.
        log_entry = {
            "timestamp": datetime.utcnow().isoformat() + "Z",
            "level": "warn",
            "service": "backend-python-agent",
            "component": "memory_client",
            "method": "store_session_history",
            "session_id": session_id,
            "error": e.__class__.__name__,
            "details": str(e),
        }
        print(json.dumps(log_entry))
        return False


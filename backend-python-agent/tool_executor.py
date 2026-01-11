import os
from typing import Any, Dict

import requests


def _build_url(base_url: str, path: str) -> str:
    return base_url.rstrip("/") + path


def execute_tool(tool_name: str, args: dict) -> Dict[str, Any]:
    """Call the Rust Sandbox tool execution endpoint.

    Sends a POST request to `{RUST_SANDBOX_URL}/execute-tool` with JSON body:
      {"tool_name": tool_name, "args": args}

    Returns the sandbox JSON response payload. If the request fails, returns a
    structured error payload.
    """

    rust_sandbox_url = os.environ.get("RUST_SANDBOX_URL", "http://localhost:8004")
    timeout_seconds = float(os.environ.get("REQUEST_TIMEOUT_SECONDS", "2"))

    payload = {"tool_name": tool_name, "args": args}

    primary_url = _build_url(rust_sandbox_url, "/execute-tool")
    fallback_url = _build_url(rust_sandbox_url, "/api/v1/execute_tool")

    try:
        resp = requests.post(primary_url, json=payload, timeout=timeout_seconds)
        if resp.status_code == 404:
            # Backwards-compatible fallback for older Rust Sandbox route.
            resp = requests.post(fallback_url, json=payload, timeout=timeout_seconds)

        resp.raise_for_status()
        return resp.json()
    except requests.exceptions.RequestException as e:
        return {
            "error": "rust_sandbox_connection_error",
            "details": str(e),
            "url": primary_url,
        }
    except ValueError as e:
        # JSON decode error
        return {
            "error": "rust_sandbox_invalid_json",
            "details": str(e),
            "url": primary_url,
        }


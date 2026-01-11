from __future__ import annotations

import json
from typing import Optional


def parse_tool_call(llm_plan_json: str) -> Optional[tuple[str, dict]]:
    """Parse a tool call from the LLM plan JSON string.

    Supports BOTH formats for compatibility:
    
    Format 1 (Go Agent Planner / Model Gateway):
      {
        "tool": {
          "name": "tool_name",
          "args": {"arg1": "value"}
        }
      }

    Format 2 (Legacy Python Agent):
      {
        "tool_call": {
          "name": "tool_name",
          "arguments": {"arg1": "value"}
        }
      }

    Returns:
      (tool_name, args) if present, else None.
    """

    if not llm_plan_json:
        return None

    try:
        data = json.loads(llm_plan_json)
    except json.JSONDecodeError:
        return None

    if not isinstance(data, dict):
        return None

    # Try Format 1: {"tool": {"name": ..., "args": ...}} (Go Agent Planner)
    tool_obj = data.get("tool")
    if isinstance(tool_obj, dict):
        name = tool_obj.get("name")
        args = tool_obj.get("args")
        
        if isinstance(name, str) and name.strip():
            if args is None:
                args = {}
            if isinstance(args, dict):
                return name.strip(), args

    # Try Format 2: {"tool_call": {"name": ..., "arguments": ...}} (Legacy)
    tool_call = data.get("tool_call")
    if isinstance(tool_call, dict):
        name = tool_call.get("name")
        args = tool_call.get("arguments")

        if isinstance(name, str) and name.strip():
            if args is None:
                args = {}
            if isinstance(args, dict):
                return name.strip(), args

    return None

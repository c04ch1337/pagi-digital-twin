from __future__ import annotations

import json
from typing import Any, Callable, Dict, Optional

import grpc

from grpc_client import get_llm_plan
from memory_client import get_session_history
from tool_parser import parse_tool_call
from instrumentation import tracer


class AgentLoop:
    def __init__(
        self,
        tool_executor: Optional[Callable[[str, dict], dict]] = None,
        max_turns: int = 3,
    ):
        self._tool_executor = tool_executor
        self._max_turns = max_turns

    async def run_agent(self, prompt: str, context: dict) -> Dict[str, Any]:
        """Run the (iterative) agent loop.

        Structure (future): retrieve memory -> call LLM -> optionally call tools -> repeat.
        Current behavior: loop for up to `max_turns`, executing any parsed tool calls and
        augmenting the prompt with `<tool_response>` blocks between turns.
        """

        with tracer.start_as_current_span("AgentLoop.run_agent") as span:
            span.set_attribute("agent.max_turns", self._max_turns)
            span.set_attribute("agent.prompt", prompt)

            session_id = (
                context.get("session_id")
                or context.get("request_id")
                or "session-unknown"
            )
            span.set_attribute("agent.session_id", session_id)

            history: list[dict] = []
            llm_response: dict = {}
            current_prompt: str = prompt

            for turn in range(self._max_turns):
                span.set_attribute("agent.turn", turn)

                # 1) RETRIEVE KNOWLEDGE/MEMORY (Placeholder)
                if turn == 0:
                    with tracer.start_as_current_span("Memory.get_session_history") as mem_span:
                        mem_span.set_attribute("memory.session_id", session_id)
                        history_messages = await get_session_history(session_id)

                    history_str = json.dumps(history_messages)
                    preamble = (
                        f"<history session_id='{session_id}'>\n"
                        f"{history_str}\n"
                        f"</history>\n\n"
                    )
                    current_prompt = preamble + prompt

                # 2) CALL LLM FOR PLAN/ACTION
                with tracer.start_as_current_span("ModelGateway.get_llm_plan"):
                    try:
                        llm_response = await get_llm_plan(current_prompt)
                    except grpc.aio.AioRpcError as e:
                        llm_response = {
                            "error": "grpc_error",
                            "code": e.code().name if hasattr(e, "code") else "unknown",
                            "details": e.details() if hasattr(e, "details") else str(e),
                        }
                    except Exception as e:
                        llm_response = {"error": e.__class__.__name__, "details": str(e)}

                history.append(
                    {
                        "turn": turn,
                        "type": "llm_plan",
                        "prompt": current_prompt,
                        "llm_response": llm_response,
                    }
                )

                # 3) IF LLM RETURNS TOOL CALL:
                tool_call = parse_tool_call(llm_response.get("plan", "{}"))
                if tool_call:
                    tool_name, args = tool_call
                    if not self._tool_executor:
                        history.append(
                            {
                                "turn": turn,
                                "type": "tool_result",
                                "tool_name": tool_name,
                                "args": args,
                                "tool_result": {
                                    "error": "no_tool_executor",
                                    "details": "AgentLoop was not initialized with a tool executor.",
                                },
                            }
                        )
                        break

                    with tracer.start_as_current_span("Tools.execute") as tool_span:
                        tool_span.set_attribute("tool.name", tool_name)
                        tool_result = self._tool_executor(tool_name, args)

                    history.append(
                        {
                            "turn": turn,
                            "type": "tool_result",
                            "tool_name": tool_name,
                            "args": args,
                            "tool_result": tool_result,
                        }
                    )

                    tool_output_str = json.dumps(tool_result)
                    current_prompt += (
                        f"\n\n<tool_response tool='{tool_name}'>\n"
                        f"{tool_output_str}\n"
                        f"</tool_response>"
                    )

                    # Continue the loop for another LLM turn with augmented context.
                    continue

                # 4) IF LLM RETURNS FINAL ANSWER: (not implemented yet)
                # 5) IF MAX TURNS REACHED:

                # For now: if no tool was requested, assume the LLM is done.
                break

            return {
                "agent": {"turns_executed": len(history), "max_turns": self._max_turns},
                "plan": {"prompt": prompt, "llm_plan": llm_response.get("plan")},
                "llm_response": llm_response,
                "history": history,
                "session_id": session_id,
            }


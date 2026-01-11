# Core Agent/Planner Loop (Pseudocode)

This describes the high-level control flow of the Agent loop that orchestrates:

- Go Model Gateway (LLM planner)
- Python Memory Service (Chroma: Domain/Body/Soul KBs; SQLite: Episodic/Heart KBs)
- Rust Sandbox (tool execution)

## `AgentLoop(prompt, session_id)`

```text
function AgentLoop(prompt, session_id):
    max_turns = N
    turn = 0

    while turn < max_turns:
        turn += 1

        # 1) MEMORY: Session history (Episodic-KB / Heart-KB)
        #    -> Python Memory Service (SQLite)
        history = MemoryService.GetSessionHistory(session_id)

        # 2) MEMORY: RAG retrieval (Domain-KB / Body-KB / Soul-KB)
        #    -> Python Memory Service (Chroma)
        kb_list = ["Domain-KB", "Body-KB", "Soul-KB"]
        rag_matches = MemoryService.GetRAGContext(
            query = prompt,
            top_k = K,
            knowledge_bases = kb_list
        )

        # 3) CONTEXT ASSEMBLY
        #    Build a single prompt/context bundle for the planner.
        planner_input = BuildPlannerPrompt(
            user_prompt = prompt,
            session_history = history,
            rag_context = rag_matches
        )

        # 4) PLANNING
        #    -> Go Model Gateway (LLM)
        plan = ModelGateway.GetPlan(prompt = planner_input)
        # plan is structured (JSON) and may include a tool call request.

        # 5) EXECUTION CHECK
        tool_call = TryParseToolCall(plan)
        if tool_call is None:
            # 5a) FINAL RESPONSE PATH
            final_answer = ExtractFinalAnswer(plan)

            # 6) PERSIST (store final step)
            MemoryService.StoreSessionHistory(
                session_id = session_id,
                history_delta = [{"role":"user","content":prompt}, {"role":"assistant","content":final_answer}]
            )

            return final_answer

        # 5b) TOOL EXECUTION PATH
        tool_result = RustSandbox.ExecuteTool(
            tool_name = tool_call.name,
            args = tool_call.args
        )

        # 6) FEEDBACK / LOOP
        #    Feed tool output back into the next planning turn.
        #    The next prompt is augmented with tool outputs.
        prompt = BuildFollowupPrompt(
            original_prompt = prompt,
            plan = plan,
            tool_result = tool_result
        )

        # 7) PERSIST (store tool event)
        MemoryService.StoreSessionHistory(
            session_id = session_id,
            history_delta = [
                {"role":"assistant","content":plan},
                {"role":"tool","content":tool_result}
            ]
        )

    # Safety fallback if max_turns reached
    return "Max turns reached; unable to complete request."
```

## Notes on KB usage

- **Domain-KB / Body-KB / Soul-KB**: queried via `GetRAGContext()` (vector similarity; Chroma)
- **Episodic-KB / Heart-KB**: loaded/stored via `GetSessionHistory()` / `StoreSessionHistory()` (SQLite)
- **Mind-KB**: loop state (turn count, tool outputs, scratchpad) maintained in-process by the Agent


# Evolving Playbooks (Mind-KB) — Conceptual Definition

This document defines the **Mind-KB** mechanism: a persistent, searchable collection of *successful multi-step sequences* ("playbooks") that can be retrieved via RAG and fed back into planning prompts to improve tool-use accuracy over time.

## 1) What is a Playbook?

A **Playbook** is an extracted slice of a session that represents a successful task completion.

It is intentionally formatted as a compact, LLM-readable sequence of steps including:

- the **original user prompt**
- one or more **planner decisions** (tool call JSON)
- the corresponding **tool results**
- the final **assistant answer** (non-tool-call completion)

### 1.1 Canonical structure (sequence form)

The playbook is stored as a sequence of role/content entries:

```json
[
  {"role": "user", "content": "<original_prompt>"},
  {"role": "assistant", "content": "<successful_plan_json_1>"},
  {"role": "tool_result", "content": "<result_json_1>"},
  {"role": "assistant", "content": "<successful_plan_json_2>"},
  {"role": "tool_result", "content": "<result_json_2>"},
  ...,
  {"role": "assistant", "content": "<final_answer_text_or_json>"}
]
```

### 1.2 Dense text format (document form)

For vector search, the sequence is summarized into a dense text document:

```
Playbook for: <original_prompt>
---
Steps:
1) Planner decided: <tool call JSON>
2) Tool returned: <tool result JSON>
...
N) Final answer: <final answer>
```

This format is intentionally redundant and structured so the LLM can quickly pattern-match the sequence.

## 2) Mind-KB: Storage + Retrieval

### 2.1 New knowledge base name

Mind-KB is represented as a Chroma collection:

```
collection name: "Mind-KB"
kind metadata: "playbook"
```

### 2.2 Storage requirements

- **Frequent writes**: Mind-KB is appended to as tasks succeed.
- **Searchable**: Mind-KB is queried similarly to other KBs for RAG.
- **Deduplication**: identical playbooks should not be stored repeatedly.

Suggested ID strategy:

- `sha256(playbook_text)`

Suggested metadata:

- `source_session`: originating session ID
- `original_prompt`: user prompt
- `kind`: `"playbook"`

## 3) Agent Planner learning loop (conceptual)

### 3.1 Success heuristic

The Go Agent Planner should treat a session as a "success" when:

- it exits the loop early (`turn < max_turns`), **and**
- the final plan is a **non-tool** response (i.e., no `{"tool": ...}` object), **and**
- there was at least one tool-use step (otherwise there is no playbook to learn)

### 3.2 Commit logic

On success, the planner should extract the minimal relevant slice:

- the original user prompt
- the set of tool-plan / tool-output pairs that led to success
- the final completion

Then call the Memory Service "Mind-KB storage method" with:

- `session_id`
- `prompt`
- `history_sequence` (the canonical sequence form)

### 3.3 Retrieval usage (future)

Before calling the LLM for planning, the planner can query Mind-KB with the current prompt and include the best-matching playbooks in the planning prompt:

```
<mind_kb_playbooks>
... retrieved playbooks ...
</mind_kb_playbooks>
```


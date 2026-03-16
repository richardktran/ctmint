# Cycle 8 — Plugins and Capabilities (Tool Mesh)

## Goal

Introduce a **capability-based** tool surface so that the LLM (and planner) see only a **relevant subset** of tools for each request, avoiding context explosion and tool confusion. Implement **Tool Registry**, **Capability Router**, and treat built-in code/logs/traces/database tools as **plugins** that register under capabilities (code, logs, traces, database). Optional: support **external MCP server** discovery and invocation.

---

## Prerequisites

- Cycle 2–7: All MCP tools (get_architecture_map, get_service_graph, get_code_snippet, get_function_summary, search_code, get_db_schema, search_logs, query_traces, explain_service) exist and work.
- Orchestrator (or at least the MCP server) can invoke tools by name and pass arguments.

---

## Tasks (Step-by-Step)

### 1. Plugin manifest schema

- [ ] Define **plugin manifest** (per plugin):
  - `name` (string, unique)
  - `version` (optional)
  - `capabilities`: list of strings (e.g. `["code"]`, `["logs", "traces"]`)
  - `tools`: list of { name, description, input_schema (JSON Schema or equivalent) }
  - Optional: `config_schema` for per-project config
- [ ] Document the schema; add a sample manifest for “built-in code plugin.”

### 2. Built-in plugins as manifests

- [ ] Create **static manifests** for built-in functionality:
  - **code** plugin: capabilities = ["code"], tools = get_architecture_map, get_service_graph, get_function_summary, get_code_snippet, search_code, explain_service.
  - **logs** plugin: capabilities = ["logs"], tools = search_logs.
  - **traces** plugin: capabilities = ["traces"], tools = query_traces.
  - **database** plugin: capabilities = ["database"], tools = get_db_schema, get_db_usage (if implemented).
- [ ] Register these in code (no external file required for built-ins); they are the “first plugins.”

### 3. Tool Registry

- [ ] **Tool Registry** (in-memory or persistent):
  - **register(manifest)**: For each tool, index by tool name → (plugin_id, input_schema, description). For each capability, index capability → list of (plugin_id, tool_name).
  - **get_tools_for_capabilities(capability_set)**: Return union of all tools from plugins that have any of the given capabilities. Deduplicate by tool name (first plugin wins or merge).
  - **get_tool(name)**: Return plugin_id and schema for that tool.
  - **list_capabilities()**: Return all capabilities that have at least one tool.
- [ ] On startup, **load** built-in plugin manifests and call register for each.

### 4. Capability Router (classifier)

- [ ] **Input**: user question (string).
- [ ] **Output**: set of capabilities, e.g. `["code", "logs", "traces"]`.
- [ ] **Implementation v1**: **Rule-based**:
  - Keyword rules: e.g. “error”, “failing”, “500” → add logs, traces; “slow”, “latency” → add traces, code; “schema”, “table”, “query” → add database; “explain”, “what does” → add code.
  - Default: if nothing matches, return `["code"]` so at least code tools are available.
- [ ] **Optional v2**: Call a **small LLM** with prompt “Classify this question into capabilities: code, logs, traces, database. Return JSON array.” Use result as capability set. Fallback to rules if LLM fails.
- [ ] **API**: `resolve_capabilities(question: &str) -> Vec<String>`.

### 5. Tool exposure funnel

- [ ] When the **MCP server** (or orchestrator) receives a request that includes a **user question** (e.g. for a “diagnose” or “answer” flow):
  - Call **resolve_capabilities(question)**.
  - Call **get_tools_for_capabilities(capabilities)**.
  - **Expose only these tools** in the tool list sent to the LLM (or used by the planner). Other tools are hidden for this request.
- [ ] When the request is **generic** (e.g. “list all tools” or no question context): expose all tools or a default set (e.g. code + logs + traces + database).
- [ ] Document this behavior in the MCP tool list response (e.g. include a note “Tools filtered by capability for this session.”).

### 6. Plugin Bridge (invocation)

- [ ] **execute_tool(plugin_id, tool_name, arguments)**: 
  - For built-in plugins, **dispatch** to the existing implementation (same code path as before; plugin_id just selects which subset is “active”).
  - No change to actual tool logic; only the **list** of tools is filtered by capability.
- [ ] Optional: **external MCP server** — if plugin_id refers to an external server (e.g. URL or command), send MCP tool call to that server and return result. For v1, only built-in plugins are required.

### 7. Discovery of external plugins (optional)

- [ ] **Config**: list of external plugin servers (e.g. URL or command line). On startup, connect and fetch their manifest (via MCP “list tools” or custom “get manifest”); register each as a plugin with a unique name.
- [ ] If discovery fails (timeout, connection error), log and continue without that plugin; do not crash.
- [ ] Document how to add an external MCP server (e.g. `mcp-kubernetes`) and which capabilities it should declare.

### 8. MCP tools for introspection

- [ ] **list_capabilities()**: Return list of all capabilities (from registry). No args. Useful for UI or debugging.
- [ ] **list_tools(capability?)**: If capability given, return tools for that capability; else return all tools. Helps users see what’s available.

### 9. Tests

- [ ] Unit test: register two plugins (code and logs); get_tools_for_capabilities(["logs"]) returns only log tools; get_tools_for_capabilities(["code", "logs"]) returns both.
- [ ] Unit test: capability router with rule-based classifier; assert “Why is login failing?” yields capabilities containing logs and traces.
- [ ] Integration: send a request with question “What errors in auth-service?”; assert the tool list in the session includes search_logs and does not include get_db_schema (or include it if rules add database). No need to run LLM; just assert tool list.

---

## Deliverables

- Plugin manifest schema and Tool Registry implementation.
- Built-in plugins (code, logs, traces, database) registered with capabilities.
- Capability Router (rule-based; optional LLM).
- Tool exposure: only tools for resolved capabilities are exposed per request/session.
- Optional: external plugin discovery and invocation.
- MCP tools: list_capabilities, list_tools(capability?).
- Tests: registry, router, filtered tool list.

---

## Definition of Done

- [ ] For a debugging question like “Why is payment-service returning 500?”, the system exposes only relevant tools (e.g. logs, traces, code) and not unrelated ones (e.g. get_db_schema if the question doesn’t mention DB).
- [ ] Tool list size per request is bounded (e.g. under 15 tools) even if the system has 30+ tools total.
- [ ] Built-in tools still work when invoked; capability only affects **visibility** to the LLM/planner.

---

## Acceptance Criteria

- Diagnose flow (Cycle 9) can use the capability router to get a small tool set for the user question and pass it to the planner/LLM.
- A client can call list_capabilities and list_tools and see the same structure as in the design doc.

---

## Estimated Duration

5–10 days.

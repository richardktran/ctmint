# Plugins and Capabilities – Technical Design (ContextMint)

## 1. Purpose and Scope

The **plugin system** allows ContextMint to extend its tool surface without hardcoding every integration. Plugins can be **in-process** (same binary, loaded modules) or **external** (separate MCP servers). To avoid **context explosion**—exposing hundreds of tools to the LLM at once—plugins register **capabilities** (e.g. `logs`, `traces`, `code`, `database`). The **Capability Router** (in the Orchestrator) selects which plugins and tools to expose for a given user question, so the LLM sees only a **relevant subset** of tools.

This document covers: capability model, plugin manifest, discovery, routing, and how this fits into the single-binary and multi-service deployment.

---

## 2. Core Idea: Capability-Based Exposure

Instead of:

- “Expose all tools from all plugins to the LLM” (leads to huge context and poor tool choice),

we do:

- “Classify the user’s intent into **capabilities** (e.g. logs, traces, code, database); then expose **only** the tools that belong to those capabilities.”

So the flow is:

```text
User question
   │
   ▼
Capability classifier (small LLM or rules)
   │
   ▼
Set of capabilities, e.g. { logs, traces, code }
   │
   ▼
Capability Router → which plugins and tools to expose
   │
   ▼
Tool Registry → filtered tool list for planner / LLM
```

---

## 3. Capability Taxonomy

A **capability** is a coarse label that describes what kind of system aspect the user might need. Suggested set:

| Capability   | Description                     | Example tools / plugins        |
|-------------|----------------------------------|--------------------------------|
| code        | Source code, symbols, navigation | get_function_summary, get_code_snippet, search_symbol |
| logs        | Log search and aggregation      | search_logs, tail_logs, get_error_rate |
| traces      | Distributed traces              | query_traces, get_trace_graph, get_latency |
| database    | Schema and usage                | get_db_schema, get_db_usage, explain_query |
| metrics     | Time-series metrics             | get_metric, get_latency_p99    |
| network     | Network flows, topology         | get_flows, get_network_topology (optional) |
| kubernetes  | K8s resources and status       | get_pods, get_events           |
| deployment  | CI/CD, releases                 | get_deployments, get_releases  |
| security    | Vulnerabilities, secrets        | scan_vulns, list_secrets      |
| cost        | Cost and usage                  | get_cost_by_service            |

New capabilities can be added without changing the core routing logic; new plugins register under existing or new capability names.

---

## 4. Plugin Manifest

Each plugin provides a **manifest** that declares its identity and what it offers.

**Structure** (conceptual):

```json
{
  "name": "logs-plugin",
  "version": "1.0.0",
  "capabilities": ["logs"],
  "tools": [
    {
      "name": "search_logs",
      "description": "Search logs by service, query, and time range.",
      "input_schema": { ... }
    },
    {
      "name": "tail_logs",
      "description": "Stream recent log lines for a service.",
      "input_schema": { ... }
    }
  ],
  "config_schema": {
    "log_store_path": "string",
    "max_lines": "number"
  }
}
```

**Fields**:
- `name`: Unique plugin identifier.
- `capabilities`: List of capability labels this plugin serves.
- `tools`: List of tools (name, description, input_schema) that the plugin implements.
- Optional: `config_schema` for per-project or global config.

**Discovery**: At startup (or on demand), the Orchestrator loads manifests from:
- A **plugins** directory (e.g. `./plugins/*.json` or `*.plugin`).
- Or from **external MCP servers** that advertise their manifest via the MCP protocol (e.g. “list tools” + a convention for capability tags in tool metadata).

---

## 5. Tool Registry

The **Tool Registry** is the central index inside the Orchestrator.

**Data structures**:
- **By capability**: `capability → list of (plugin_id, tool_name)`.
- **By tool name**: `tool_name → (plugin_id, input_schema, description)`.
- **By plugin**: `plugin_id → manifest` (for config and lifecycle).

**Operations**:
- **Register(manifest)**: For each capability in the manifest, append (plugin_id, tool_name) for each tool; index by tool name.
- **GetToolsForCapabilities(capability_set)**: Return the union of all tools whose plugin has at least one of the given capabilities; deduplicate by tool name (first plugin wins or merge).
- **GetTool(name)**: Return plugin_id and schema for invocation.
- **ListCapabilities()**: Return all capabilities that have at least one tool (for introspection and UI).

---

## 6. Capability Router and Classification

**Input**: User question (and optionally conversation history or current project).

**Output**: Set of capabilities, e.g. `{"logs", "traces", "code"}`.

**Implementation options**:

1. **Rule-based**: Keywords or patterns (e.g. “error”, “failing” → logs + traces; “slow” → traces + code + database; “schema” → database).
2. **Small LLM**: Local model (e.g. via Ollama) with prompt: “Classify the following user question into one or more capabilities: code, logs, traces, database, metrics, network, kubernetes, deployment, security, cost.” Output: JSON array of capability strings.
3. **Hybrid**: Rules for obvious cases; LLM for ambiguous or long questions.

The router runs **before** building the tool list for the planner, so the rest of the pipeline (planning, execution, context funnel) only sees the filtered tools.

---

## 7. Tool Execution and Plugin Bridge

When the Orchestrator decides to call a tool (e.g. `search_logs`):

1. **Dispatch**: Tool Registry returns that `search_logs` is implemented by `logs-plugin`.
2. **Invoke**:
   - **In-process plugin**: Call a function or method (e.g. Rust module implementing a `Plugin` trait) with the tool name and arguments.
   - **External MCP server**: Send MCP “tools/call” request to the plugin’s endpoint (stdio or HTTP) with tool name and arguments.
3. **Response**: Normalize success/error and return to the planner or Context Funnel.

**Plugin Bridge** abstracts over in-process vs external: same interface “execute(plugin_id, tool_name, arguments) → result”.

---

## 8. Single Binary and In-Process Plugins

For the **single-binary** deployment, “plugins” can be **built-in modules** that implement the same manifest + tool contract:

- **Built-in capabilities**: code, logs, traces, database (and optionally metrics) are implemented inside the binary; they read from SKG, vector index, and runtime stores populated by the ingestion pipelines.
- **Manifest**: Each built-in module has a static manifest (e.g. in code or embedded JSON) and registers at startup.
- **No external process**: No subprocess or HTTP call for these; just function calls.

**Extension**:
- **Dynamic plugins**: If the binary supports loading dynamic libraries (e.g. `.so`/`.dylib`), a plugin could implement a C ABI or Rust trait and register its manifest and callbacks. This is optional and more complex; v1 can ship with only built-in capabilities.
- **External MCP**: For advanced users, the binary can also connect to **external** MCP servers (e.g. a custom `mcp-kubernetes` server) and merge their manifests into the Tool Registry under their declared capabilities.

---

## 9. Context Funnel and Tool Results

The Context Funnel receives **raw tool results** (e.g. log lines, trace JSON, code snippets) and compresses them into a **ContextBundle** for the LLM. So plugins do not need to “summarize”; they return structured data, and the funnel is responsible for truncation, summarization, and token budgeting. Plugins should still respect **limits** (e.g. max lines, max traces) from the Orchestrator so that the funnel is not overwhelmed.

---

## 10. Security and Isolation

- **Per-project**: Tools that accept `project_id` should only return data for that project (enforced by the plugin or by the Orchestrator passing scoped credentials).
- **Sensitive data**: Log and trace tools should redact or mask secrets if configured; plugin config can define redaction rules.
- **External plugins**: When calling external MCP servers, the Orchestrator should not forward raw credentials; use scoped tokens or proxy queries through the core.

---

## 11. Observability

- **Registry**: Log at startup which plugins and how many tools per capability were loaded.
- **Routing**: Optional log of “question → capabilities → tools exposed” for debugging.
- **Execution**: Trace tool calls (plugin_id, tool_name, duration, success/failure) for observability and cost control.

---

## 12. Summary

The **capability-based plugin system** keeps the LLM’s tool set small and relevant by:
- Classifying the user question into **capabilities**.
- Exposing **only** tools from plugins that implement those capabilities.
- Using a **Tool Registry** and **Plugin Bridge** (in-process or MCP) to discover and execute tools.

This enables a **single binary** to ship with built-in code/logs/traces/database capabilities while remaining **extensible** via external MCP servers and optional dynamic plugins, without context explosion or tool confusion.
